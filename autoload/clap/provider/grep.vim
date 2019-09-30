" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Grep on the fly with smart cache strategy in async way.

let s:save_cpo = &cpo
set cpo&vim

let s:grep_delay = get(g:, 'clap_provider_grep_delay', 300)
let s:grep_blink = get(g:, 'clap_provider_grep_blink', [2, 100])

let s:old_query = ''
let s:grep_timer = -1

let s:NO_MATCHES = 'NO MATCHES FOUND'

if has('nvim')

  let s:default_prompt = "Type anything you want to find"

  function! s:apply_append_or_cache(data) abort
    let data = a:data

    " Here are dragons!
    let line_count = g:clap.display.line_count()

    " Reach the preload capacity for the first time
    " Append the minimum data, the rest goes to the cache.
    if len(data) + line_count >= g:clap.display.preload_capacity
      let start = g:clap.display.preload_capacity - line_count
      let to_append = data[:start-1]
      let to_cache = data[start:]

      " Discard?
      call extend(g:clap.display.cache, to_cache)

      let to_append = map(to_append, 's:draw_icon(v:val)')
      call g:clap.display.append_lines(to_append)

      let s:preload_is_complete = v:true
      let s:loaded_size = line_count + len(to_append)
    else
      let s:loaded_size = line_count + len(data)
      let data = map(data, 's:draw_icon(v:val)')
      call g:clap.display.append_lines(data)
    endif
  endfunction

  function! s:append_output(data) abort
    if empty(a:data)
      return
    endif

    if s:preload_is_complete
      call extend(g:clap.display.cache, a:data)
    else
      call s:apply_append_or_cache(a:data)
    endif

    let matches_count = s:loaded_size + len(g:clap.display.cache)

    call clap#indicator#set_matches('['.matches_count.']')
  endfunction

  function! s:on_event(job_id, data, event) abort
    if a:event == 'stdout'
      if len(a:data) > 1
        " Second last is the real last one for neovim.
        call s:append_output(a:data[:-2])
      endif
    elseif a:event == 'stderr'
      " Ignore the errors?
    else
      call s:check_if_no_matches()
      let g:clap.is_busy = 0
    endif
  endfunction

  function! s:job_start(cmd) abort
    let s:jobid = jobstart(a:cmd, {
          \ 'on_exit': function('s:on_event'),
          \ 'on_stdout': function('s:on_event'),
          \ 'on_stderr': function('s:on_event'),
          \ })
  endfunction

else

  let s:default_prompt = "Search ??"

  function! s:append_output(preload) abort
    let to_append = a:preload
    let to_append = map(to_append, 's:draw_icon(v:val)')
    call g:clap.display.append_lines(to_append)
    let s:loaded_size = len(to_append)
    let s:preload_is_complete = v:true
    let s:did_preload = v:true
  endfunction

  function! s:post_check() abort
    if !s:preload_is_complete
      call s:append_output(s:vim_output)
    endif
    call s:check_if_no_matches()
    call clap#spinner#set_idle()
    call s:update_indicator()
  endfunction

  function! s:out_cb(channel, message) abort
    if s:preload_is_complete
      call add(g:clap.display.cache, a:message)
    else
      call add(s:vim_output, a:message)
      if len(s:vim_output) >= g:clap.display.preload_capacity
        call s:append_output(s:vim_output)
      endif
    endif
  endfunction

  function! s:err_cb(channel, message) abort
    call g:clap.abort(channel.", ".a:message)
  endfunction

  function! s:close_cb(_channel) abort
    call s:post_check()
  endfunction

  function! s:exit_cb(_job, _exit_code) abort
    call s:post_check()
  endfunction

  function! s:job_start(cmd) abort
    let s:jobid = job_start(['bash', '-c', a:cmd], {
          \ 'in_io': 'null',
          \ 'err_cb': function('s:err_cb'),
          \ 'out_cb': function('s:out_cb'),
          \ 'exit_cb': function('s:exit_cb'),
          \ 'close_cb': function('s:close_cb'),
          \ 'noblock': 1,
          \ })
  endfunction

endif

function! s:update_indicator() abort
  if s:preload_is_complete
    let matches_count = s:loaded_size + len(g:clap.display.cache)
  else
    let matches_count = g:clap.display.line_count()
  endif

  call clap#indicator#set_matches('['.matches_count.']')
endfunction

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

function! s:check_if_no_matches() abort
  if g:clap.display.is_empty()
    call g:clap.display.set_lines([s:NO_MATCHES])
    call clap#indicator#set_matches('[0]')
  endif
endfunction

function! s:cmd(query) abort
  if !executable('rg')
    call g:clap.provider.abort('rg not found')
    return
  endif
  let cmd = 'rg -H --no-heading --vimgrep --smart-case "'.a:query.'"'
  let g:clap.provider.cmd = cmd
  return cmd
endfunction

function! s:jobstop() abort
  if exists('s:jobid')
    if has('nvim')
      silent! call jobstop(s:jobid)
    else
      silent! call jobstop(s:jobid, 'kill')
    endif
    unlet s:jobid
  endif
endfunction

function! s:clear_job_and_matches() abort
  call s:jobstop()

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

  let s:cache_size = 0
  let s:loaded_size = 0
  let s:preload_is_complete = v:false

  let s:vim_output = []
  let g:clap.display.cache = []

  call s:job_start(s:cmd(query))

  " Add an option for highlighting the query string?
  " let w:clap_query_hi_id = matchaddpos('ClapQuery', [1])

  " Consistent with --smart-case of rg
  " Searches case insensitively if the pattern is all lowercase. Search case sensitively otherwise.
  let ignore_case = query =~ '\u' ? '\C' : '\c'
  let pattern = ignore_case.'^.*\d\+:\d\+:.*\zs'.query

  call g:clap.display.add_highlight(pattern)

  call clap#spinner#set_busy()
endfunction

function! s:grep_exit() abort
  call s:jobstop()
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

  if exists('g:__clap_action')
    execute clap#action_for(g:__clap_action) fpath
    unlet g:__clap_action
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

let s:grep.converter = function('s:draw_icon')

let s:grep.jobstop = function('s:jobstop')

let s:grep.on_exit = function('s:grep_exit')

let g:clap#provider#grep# = s:grep

let &cpo = s:save_cpo
unlet s:save_cpo
