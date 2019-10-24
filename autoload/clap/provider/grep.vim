" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Grep on the fly with smart cache strategy in async way.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:grep_delay = get(g:, 'clap_provider_grep_delay', 300)
let s:grep_blink = get(g:, 'clap_provider_grep_blink', [2, 100])
let s:grep_opts = get(g:, 'clap_provider_grep_opts', '')

let s:old_query = ''
let s:grep_timer = -1

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
  if !executable('rg')
    call g:clap.abort('rg not found')
    return
  endif
  let cmd = 'rg -H --no-heading --vimgrep --smart-case '.s:grep_opts.' "'.a:query.'"'.(has('win32') ? ' .' : '')
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

  let s:old_pos = getcurpos()
  let s:old_query = query

  " Clear the previous search result and reset cache.
  " This should happen before the new job.
  call g:clap.display.clear()

  call clap#dispatcher#job_start(s:cmd(query))

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

function! s:grep_sink(selected) abort
  call s:grep_exit()
  let line = a:selected

  let pos_pattern = '\(.*\):\(\d\+\):\(\d\+\):'
  if get(s:, 'icon_appended', v:false)
    let matched = matchlist(line, '^.* '.pos_pattern)
  else
    let matched = matchlist(line, '^'.pos_pattern)
  endif

  let [fpath, linenr, column] = [matched[1], str2nr(matched[2]), str2nr(matched[3])]
  let s:icon_appended = v:false

  " NOTE: Important!
  call g:clap.start.goto_win()

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

function! s:grep_sink_star(lines) abort
  call s:grep_exit()
  let qflist = []
  for line in a:lines
    let pos_pattern = '\(.*\):\(\d\+\):\(\d\+\):\(.*\)'
    if get(s:, 'icon_appended', v:false)
      let matched = matchlist(line, '^.* '.pos_pattern)
    else
      let matched = matchlist(line, '^'.pos_pattern)
    endif
    let [fpath, linenr, column, text] = [matched[1], str2nr(matched[2]), str2nr(matched[3]), matched[4]]
    call add(qflist, {'filename': fpath, 'lnum': linenr, 'col': column, 'text': text})
  endfor
  let s:icon_appended = v:false
  call setqflist(qflist)
  call g:clap.start.goto_win()
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
