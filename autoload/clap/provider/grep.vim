" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Grep on the fly with smart cache strategy in async way.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:grep_delay = get(g:, 'clap_provider_grep_delay', 300)
let s:grep_blink = get(g:, 'clap_provider_grep_blink', [2, 100])
let s:grep_opts = get(g:, 'clap_provider_grep_opts', '-H --no-heading --vimgrep --smart-case')
let s:grep_executable = get(g:, 'clap_provider_grep_executable', 'rg')
let s:grep_cmd_format = get(g:, 'clap_provider_grep_cmd_format', '%s %s "%s"'.(has('win32') ? ' .' : ''))

let s:old_query = ''
let s:grep_timer = -1
let s:icon_appended = v:false

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

function! s:cmd(query) abort
  if !executable(s:grep_executable)
    call g:clap.abort(s:grep_executable . ' not found')
    return
  endif
  if has_key(g:clap.context, 'opt')
    let grep_opts = s:grep_opts.' '.g:clap.context.opt
  else
    let grep_opts = s:grep_opts
  endif

  if !empty(g:clap.provider.args)
    let dir = g:clap.provider.args[-1]
    if isdirectory(expand(dir))
      let g:__clap_provider_cwd = dir
    endif
  endif

  let cmd = printf(s:grep_cmd_format, s:grep_executable, grep_opts, a:query)
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

  let s:preview_cache = {}
  let s:old_query = query

  " Clear the previous search result and reset cache.
  " This should happen before the new job.
  call g:clap.display.clear()

  call clap#rooter#try_set_cwd()

  call clap#rooter#run(function('clap#dispatcher#job_start'), s:cmd(query))

  " Consistent with --smart-case of rg
  " Searches case insensitively if the pattern is all lowercase. Search case sensitively otherwise.
  let ignore_case = query =~# '\u' ? '\C' : '\c'
  let pattern = ignore_case.'^.*\d\+:\d\+:.*\zs'.query

  call g:clap.display.add_highlight(pattern)

  call clap#spinner#set_busy()
endfunction

function! s:grep_exit() abort
  call clap#dispatcher#jobstop()
endfunction

function! s:matchlist(line, pattern) abort
  if s:icon_appended
    return matchlist(a:line, '^.* '.a:pattern)
  else
    return matchlist(a:line, '^'.a:pattern)
  endif
endfunction

function! s:grep_on_move() abort
  let pattern = '\(.*\):\(\d\+\):\(\d\+\):'
  let cur_line = g:clap.display.getcurline()
  let matched = s:matchlist(cur_line, pattern)
  try
    let [fpath, lnum] = [matched[1], str2nr(matched[2])]
  catch
    return
  endtry
  if !has_key(s:preview_cache, fpath)
    if filereadable(expand(fpath))
      let s:preview_cache[fpath] = {
            \ 'lines': readfile(expand(fpath), ''),
            \ 'filetype': clap#ext#into_filetype(fpath)
            \ }
    else
      echom fpath.' is unreadable'
      return
    endif
  endif
  let [start, end, hi_lnum] = clap#util#get_preview_line_range(lnum, 5)
  let preview_lines = s:preview_cache[fpath]['lines'][start : end]
  call g:clap.preview.show(preview_lines)
  call g:clap.preview.load_syntax(s:preview_cache[fpath].filetype)
  call g:clap.preview.add_highlight(hi_lnum)
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
  call setqflist(map(a:lines, 's:into_qf_item(v:val, pattern)'))
  let s:icon_appended = v:false
  copen
  cc
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

let s:grep.on_enter = { -> g:clap.display.setbufvar('&ft', 'clap_grep') }

if get(g:, 'clap_provider_grep_enable_icon',
      \ exists('g:loaded_webdevicons')
      \ || get(g:, 'spacevim_nerd_fonts', 0))
  let s:grep.converter = function('s:draw_icon')
endif

let s:grep.on_exit = function('s:grep_exit')

let s:grep.support_open_action = v:true
let s:grep.enable_rooter = v:true

let g:clap#provider#grep# = s:grep

let &cpoptions = s:save_cpo
unlet s:save_cpo
