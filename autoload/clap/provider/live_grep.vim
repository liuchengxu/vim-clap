" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Grep on the fly with smart cache strategy in async way.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:PATH_SEPARATOR = has('win32') ? '\' : '/'
let s:DEFAULT_PROMPT = has('nvim') ? 'Type anything you want to find' : 'Search ??'

let s:grep_delay = get(g:, 'clap_provider_live_grep_delay', 300)
let s:grep_blink = get(g:, 'clap_provider_live_grep_blink', [2, 100])
let s:grep_opts = get(g:, 'clap_provider_live_grep_opts', '-H --no-heading --vimgrep --smart-case --color=never')
let s:grep_executable = get(g:, 'clap_provider_live_grep_executable', 'rg')
let s:grep_cmd_format = get(g:, 'clap_provider_live_grep_cmd_format', '%s %s "%s"'.(has('win32') ? ' .' : ''))
let g:clap_provider_live_grep_enable_icon = get(g:, 'clap_provider_live_grep_enable_icon', g:clap_enable_icon)
let s:grep_enable_icon = g:clap_provider_live_grep_enable_icon

let s:old_query = ''
let s:grep_timer = -1
let s:icon_appended = v:false

" Translate `[query] *.rs` to `[query] -g '*.rs'` for rg.
function! s:translate_query_and_opts(query) abort
  if has_key(g:clap.context, 'opt')
    let grep_opts = s:grep_opts.' '.g:clap.context.opt
  else
    let grep_opts = s:grep_opts
  endif

  " Exact mode
  if a:query[0] ==# "'"
    return [grep_opts, a:query[1:]]
  endif

  let s:ripgrep_glob = v:null
  let ridx = strridx(a:query, ' ')
  if ridx == -1
    let query = a:query
  else
    " .vim => -g '*.vim'
    if a:query[ridx+1:] =~# '^\.\a\+'
      let ft = matchstr(a:query[ridx+1:], '^\.\zs\(.\+\)')
      if clap#maple#is_available()
        let s:ripgrep_glob = '*.'.ft
      else
        let grep_opts .= ' -g "*.'.ft.'"'
      endif
      let query = a:query[:ridx-1]
      return [grep_opts, query]
    endif

    let matched = matchlist(a:query[ridx+1:], '^\(.*\)\.\(.*\)$')
    if !empty(matched)
      if clap#maple#is_available()
        let s:ripgrep_glob = a:query[ridx+1:]
      else
        let grep_opts .= ' -g "'.a:query[ridx+1:].'"'
      endif
      let query = a:query[:ridx-1]
    else
      let query = a:query
    endif
  endif

  let query = join(split(query), '.*')

  " Consistent with --smart-case of rg
  " Searches case insensitively if the pattern is all lowercase. Search case sensitively otherwise.
  let ignore_case = query =~# '\u' ? '\C' : '\c'
  let s:hl_pattern = ignore_case.'^.*\d\+:\d\+:.*\zs'.query

  return [grep_opts, query]
endfunction

function! s:spawn(query) abort
  let query = a:query

  if s:old_query ==# query
    " Let the previous search be continued
    return
  endif

  call s:clear_job_and_matches()

  let s:icon_appended = v:false
  let s:preview_cache = {}
  let s:old_query = query

  " Clear the previous search result and reset cache.
  " This should happen before the new job.
  "
  " Do not clear the outdated content immedidately as it leads to the annoying
  " flicker.
  " call g:clap.display.clear()

  call clap#rooter#try_set_cwd()
  call s:start_job(query)
  call clap#spinner#set_busy()
endfunction

function! s:grep_exit() abort
  call clap#legacy#dispatcher#jobstop()
  let s:old_query = ''
  if exists('s:parent_dir')
    unlet s:parent_dir
  endif
  if exists('s:initial_size')
    unlet s:initial_size
  endif
endfunction

function! s:into_abs_path(fpath) abort
  if exists('s:parent_dir')
    return s:parent_dir.a:fpath
  else
    if exists('g:__clap_provider_cwd') && filereadable(g:__clap_provider_cwd.s:PATH_SEPARATOR.a:fpath)
      let s:parent_dir = g:__clap_provider_cwd.s:PATH_SEPARATOR
      return s:parent_dir.a:fpath
    elseif filereadable(getcwd().s:PATH_SEPARATOR.a:fpath)
      let s:parent_dir = getcwd().s:PATH_SEPARATOR
      return s:parent_dir.a:fpath
    elseif filereadable(clap#path#project_root_or_default(g:clap.start.bufnr).s:PATH_SEPARATOR.a:fpath)
      let s:parent_dir = clap#path#project_root_or_default(g:clap.start.bufnr).s:PATH_SEPARATOR
      return s:parent_dir.a:fpath
    endif
  endif
  return a:fpath
endfunction

function! s:grep_sink(selected) abort
  call s:grep_exit()
  let line = a:selected

  let pattern = '\(.\{-}\):\(\d\+\):\(\d\+\):'
  let matched = s:strip_icon_and_match(line, pattern)
  let [fpath, linenr, column] = [matched[1], str2nr(matched[2]), str2nr(matched[3])]
  call clap#sink#open_file(fpath, linenr, column)
  call call('clap#util#blink', s:grep_blink)
  let s:icon_appended = v:false
endfunction

function! s:into_qf_item(line, pattern) abort
  let matched = s:strip_icon_and_match(a:line, a:pattern)
  let [fpath, linenr, column, text] = [matched[1], str2nr(matched[2]), str2nr(matched[3]), matched[4]]
  return {'filename': fpath, 'lnum': linenr, 'col': column, 'text': text}
endfunction

function! s:grep_sink_star(lines) abort
  call s:grep_exit()
  let pattern = '\(.\{-}\):\(\d\+\):\(\d\+\):\(.*\)'
  call clap#util#open_quickfix(map(a:lines, 's:into_qf_item(v:val, pattern)'))
endfunction

function! s:apply_grep(_timer) abort
  let query = g:clap.input.get()
  if empty(query)
    call s:clear_job_and_matches()
    return
  endif

  try
    if has_key(g:clap.display, 'initial_size')
      let s:initial_size = g:clap.display.initial_size
      unlet g:clap.display.initial_size
    endif
    call s:spawn(query)
  catch /^vim-clap/
    call g:clap.display.set_lines([v:exception])
  endtry
endfunction

function! s:grep_on_typed() abort
  if s:grep_timer != -1
    call timer_stop(s:grep_timer)
    let s:grep_timer = -1
  endif

  if empty(g:clap.input.get())
    if exists('s:initial_size')
      call clap#indicator#set_matches_number(s:initial_size)
    elseif has_key(g:clap.display, 'initial_size')
      let s:initial_size = g:clap.display.initial_size
      unlet g:clap.display.initial_size
      call clap#indicator#set_matches_number(s:initial_size)
    else
      call clap#indicator#set(s:DEFAULT_PROMPT)
    endif
    call g:clap.display.clear_highlight()
    if exists('g:__clap_forerunner_result')
      call g:clap.display.set_lines_lazy(g:__clap_forerunner_result)
    endif
    return
  endif

  let s:grep_timer = timer_start(s:grep_delay, function('s:apply_grep'))
endfunction

function! s:grep_on_move() abort
  let pattern = '\(.*\):\(\d\+\):\(\d\+\):'
  let cur_line = g:clap.display.getcurline()
  let matched = s:strip_icon_and_match(cur_line, pattern)
  try
    let [fpath, lnum] = [expand(matched[1]), str2nr(matched[2])]
  catch
    return
  endtry

  let fpath = s:into_abs_path(fpath)
  if !filereadable(fpath)
    return
  endif

  let s:preview_cache = get(s:, 'preview_cache', {})
  if !has_key(s:preview_cache, fpath)
    let s:preview_cache[fpath] = {
          \ 'lines': readfile(expand(fpath), ''),
          \ 'filetype': clap#ext#into_filetype(fpath)
          \ }
  endif
  let [start, end, hi_lnum] = clap#preview#get_range(lnum)
  let preview_lines = s:preview_cache[fpath]['lines'][start : end]
  call insert(preview_lines, fpath)
  let hi_lnum += 1
  call clap#preview#show_lines(preview_lines, s:preview_cache[fpath].filetype, hi_lnum)
  call clap#preview#highlight_header()
endfunction

let s:grep = {}

let s:grep.icon = 'Grep'
let s:grep.syntax = 'clap_grep'
let s:grep.enable_rooter = v:true
let s:grep.support_open_action = v:true

let s:grep.sink = function('s:grep_sink')
let s:grep['sink*'] = function('s:grep_sink_star')
let s:grep.on_move = function('s:grep_on_move')
let s:grep.on_typed = function('s:grep_on_typed')
let s:grep.on_exit = function('s:grep_exit')

function! s:grep.on_move_async() abort
  call clap#client#notify('on_move')
endfunction

if clap#maple#is_available()
  function! s:grep.init() abort
    call clap#rooter#try_set_cwd()
    call clap#client#notify_on_init()
  endfunction

  function! s:clear_job_and_matches() abort
  endfunction

  function! s:start_job(query) abort
    let [grep_opts, query] = s:translate_query_and_opts(a:query)
    " Add ' .' for windows in maple
    call clap#maple#command#start_live_grep(s:grep_executable.' '.grep_opts, query, s:grep_enable_icon, s:ripgrep_glob)
  endfunction

  function! s:strip_icon_and_match(line, pattern) abort
    if g:__clap_icon_added_by_maple
      " Strip the leading icon
      return matchlist(a:line[4:], '^'.a:pattern)
    else
      return matchlist(a:line, '^'.a:pattern)
    endif
  endfunction
else
  function! s:grep.init() abort
  endfunction

  function! s:clear_job_and_matches() abort
    call clap#legacy#dispatcher#jobstop()

    call g:clap.display.clear_highlight()
  endfunction

  function! s:start_job(query) abort
    if !executable(s:grep_executable)
      call g:clap.abort(s:grep_executable . ' not found')
      return
    endif

    let [grep_opts, query] = s:translate_query_and_opts(a:query)

    let cmd = printf(s:grep_cmd_format, s:grep_executable, grep_opts, query)
    let g:clap.provider.cmd = cmd

    call clap#rooter#run(function('clap#legacy#dispatcher#job_start'), cmd)

    call g:clap.display.add_highlight(s:hl_pattern)
  endfunction

  function! s:strip_icon_and_match(line, pattern) abort
    if s:icon_appended && a:line[3] ==# ' '
      " Strip the leading icon
      return matchlist(a:line[4:], '^'.a:pattern)
    else
      return matchlist(a:line, '^'.a:pattern)
    endif
  endfunction

  if s:grep_enable_icon
    " Caveat: This function can have a peformance issue.
    function! s:draw_icon(line) abort
      let matched = matchlist(a:line, '^\(.*\):\d\+:\d\+:')
      if len(matched) > 0 && !empty(matched[1])
        let icon = clap#icon#get(matched[1])
        let s:icon_appended = v:true
        return icon.' '.a:line
      endif
      return a:line
    endfunction

    let s:grep.converter = function('s:draw_icon')
  endif
endif

let g:clap#provider#live_grep# = s:grep

let &cpoptions = s:save_cpo
unlet s:save_cpo
