" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Grep on the fly with smart cache strategy in async way.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:grep_delay = get(g:, 'clap_provider_grep_delay', 300)
let s:grep_blink = get(g:, 'clap_provider_grep_blink', [2, 100])
let s:grep_opts = get(g:, 'clap_provider_grep_opts', '-H --no-heading --vimgrep --smart-case')
let s:grep_executable = get(g:, 'clap_provider_grep_executable', 'rg')
let s:grep_cmd_format = get(g:, 'clap_provider_grep_cmd_format', '%s %s "%s"'.(has('win32') ? ' .' : ''))
let s:grep_enable_icon = get(g:, 'clap_provider_grep_enable_icon',
        \ exists('g:loaded_webdevicons') || get(g:, 'spacevim_nerd_fonts', 0))

let s:old_query = ''
let s:grep_timer = -1
let s:icon_appended = v:false

let s:path_seperator = has('win32') ? '\' : '/'

if has('nvim')
  let s:default_prompt = 'Type anything you want to find'
else
  let s:default_prompt = 'Search ??'
endif

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

function! s:cmd(query) abort
  if !executable(s:grep_executable)
    call g:clap.abort(s:grep_executable . ' not found')
    return
  endif

  let [grep_opts, query] = s:translate_query_and_opts(a:query)

  let cmd = printf(s:grep_cmd_format, s:grep_executable, grep_opts, query)
  let g:clap.provider.cmd = cmd
  return cmd
endfunction

function! s:clear_job_and_matches() abort
  call clap#dispatcher#jobstop()

  call g:clap.display.clear_highlight()
endfunction

function! s:spawn(query) abort
  let query = a:query

  if empty(query)
    call s:clear_job_and_matches()
    return
  endif

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

  if clap#maple#is_available()
    let [grep_opts, query] = s:translate_query_and_opts(a:query)
    " Add ' .' for windows in maple
    call clap#maple#run_grep(s:grep_executable.' '.grep_opts, query, s:grep_enable_icon, s:ripgrep_glob)
    if s:grep_enable_icon
      let s:icon_appended = v:true
    endif
  else
    call clap#rooter#run(function('clap#dispatcher#job_start'), s:cmd(query))
  endif

  call g:clap.display.add_highlight(s:hl_pattern)

  call clap#spinner#set_busy()
endfunction

function! s:grep_exit() abort
  call clap#dispatcher#jobstop()
  let s:old_query = ''
  if exists('s:parent_dir')
    unlet s:parent_dir
  endif
endfunction

function! s:matchlist(line, pattern) abort
  if s:icon_appended
    return matchlist(a:line, '^.* '.a:pattern)
  else
    return matchlist(a:line, '^'.a:pattern)
  endif
endfunction

function! s:into_abs_path(fpath) abort
  if exists('s:parent_dir')
    return s:parent_dir.a:fpath
  else
    if exists('g:__clap_provider_cwd') && filereadable(g:__clap_provider_cwd.s:path_seperator.a:fpath)
      let s:parent_dir = g:__clap_provider_cwd.s:path_seperator
      return s:parent_dir.a:fpath
    elseif filereadable(getcwd().s:path_seperator.a:fpath)
      let s:parent_dir = getcwd().s:path_seperator
      return s:parent_dir.a:fpath
    elseif filereadable(clap#path#project_root_or_default(g:clap.start.bufnr).s:path_seperator.a:fpath)
      let s:parent_dir = clap#path#project_root_or_default(g:clap.start.bufnr).s:path_seperator
      return s:parent_dir.a:fpath
    endif
  endif
  return a:fpath
endfunction

function! s:grep_on_move() abort
  let pattern = '\(.*\):\(\d\+\):\(\d\+\):'
  let cur_line = g:clap.display.getcurline()
  let matched = s:matchlist(cur_line, pattern)
  try
    let [fpath, lnum] = [expand(matched[1]), str2nr(matched[2])]
  catch
    return
  endtry

  let fpath = s:into_abs_path(fpath)
  if !filereadable(fpath)
    return
  endif

  if !has_key(s:preview_cache, fpath)
    let s:preview_cache[fpath] = {
          \ 'lines': readfile(expand(fpath), ''),
          \ 'filetype': clap#ext#into_filetype(fpath)
          \ }
  endif
  let [start, end, hi_lnum] = clap#preview#get_line_range(lnum, 5)
  let preview_lines = s:preview_cache[fpath]['lines'][start : end]
  call insert(preview_lines, fpath)
  let hi_lnum += 1
  call clap#preview#show_with_line_highlight(preview_lines, s:preview_cache[fpath].filetype, hi_lnum)
  call clap#preview#highlight_header()
endfunction

function! s:grep_sink(selected) abort
  call s:grep_exit()
  let line = a:selected

  let pattern = '\(.*\):\(\d\+\):\(\d\+\):'
  let matched = s:matchlist(line, pattern)
  let [fpath, linenr, column] = [matched[1], str2nr(matched[2]), str2nr(matched[3])]
  let s:icon_appended = v:false

  if has_key(g:clap, 'open_action')
    execute g:clap.open_action fpath
  else
    " Cannot use noautocmd here as it would lose syntax, and ...
    execute 'edit' fpath
  endif

  noautocmd call cursor(linenr, column)
  normal! zz
  call call('clap#util#blink', s:grep_blink)
endfunction

function! s:into_qf_item(line, pattern) abort
  let matched = s:matchlist(a:line, a:pattern)
  let [fpath, linenr, column, text] = [matched[1], str2nr(matched[2]), str2nr(matched[3]), matched[4]]
  return {'filename': fpath, 'lnum': linenr, 'col': column, 'text': text}
endfunction

function! s:grep_sink_star(lines) abort
  call s:grep_exit()
  let pattern = '\(.*\):\(\d\+\):\(\d\+\):\(.*\)'
  call clap#util#open_quickfix(map(a:lines, 's:into_qf_item(v:val, pattern)'))
endfunction

function! s:apply_grep() abort
  let query = g:clap.input.get()
  if empty(query)
    return
  endif

  try
    call s:spawn(query)
  catch /^vim-clap/
    call g:clap.display.set_lines([v:exception])
  endtry
endfunction

function! s:grep_with_delay() abort
  if s:grep_timer != -1
    call timer_stop(s:grep_timer)
    let s:grep_timer = -1
  endif

  if empty(g:clap.input.get())
    call clap#indicator#set_matches(s:default_prompt)
    call g:clap.display.clear_highlight()
    return
  endif

  let s:grep_timer = timer_start(s:grep_delay, { -> s:apply_grep() })
endfunction

let s:grep = {}

let s:grep.sink = function('s:grep_sink')

let s:grep['sink*'] = function('s:grep_sink_star')

let s:grep.on_typed = function('s:grep_with_delay')

let s:grep.on_move = function('s:grep_on_move')

let s:grep.syntax = 'clap_grep'

if !clap#maple#is_available() && s:grep_enable_icon
  let s:grep.converter = function('s:draw_icon')
endif

let s:grep.on_exit = function('s:grep_exit')

let s:grep.support_open_action = v:true
let s:grep.enable_rooter = v:true

let g:clap#provider#grep# = s:grep

let &cpoptions = s:save_cpo
unlet s:save_cpo
