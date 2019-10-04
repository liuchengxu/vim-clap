" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Handle the movement.

let s:save_cpo = &cpo
set cpo&vim

let s:old_input = ''
let s:support_multi_selection = v:false
let s:use_multi_selection = v:false

let s:lazy_load_size = 50

let s:motions = {
      \ 'up': 'k',
      \ 'down': 'j',
      \ 'left': 'h',
      \ 'right': 'l',
      \ }

function! clap#handler#on_typed() abort
  let l:cur_input = g:clap.input.get()
  if s:old_input == l:cur_input
    return
  elseif strlen(s:old_input) > strlen(l:cur_input)
    " If we should refilter?
    let g:__clap_should_refilter = v:true
  endif
  let s:old_input = l:cur_input
  call g:clap.provider.on_typed()
  if g:clap.provider.is_pure_async()
    call clap#indicator#set_matches('')
  endif
endfunction

function! s:navigate(direction) abort
  setlocal cursorline
  let curlnum = line('.')
  let lastlnum = line('$')
  if curlnum == lastlnum && a:direction ==# 'down'
    " Lazy append!
    " Append 100 more line from the cache when you need.
    if empty(g:clap.display.cache)
      normal! 1gg
      let g:__clap_display_curlnum = 1
    else
      let cache = g:clap.display.cache
      if len(cache) <= s:lazy_load_size
        let to_append = cache
        let g:clap.display.cache = []
      else
        let to_append = cache[:s:lazy_load_size-1]
        let g:clap.display.cache = cache[s:lazy_load_size:]
      endif
      if has_key(g:clap.provider._(), 'converter')
        let to_append = map(to_append, 'g:clap.provider._().converter(v:val)')
      endif
      " The buffer is not empty, qed.
      call g:clap.display.append_lines_uncheck(to_append)
      normal! j
      let g:__clap_display_curlnum += 1
    endif

  elseif curlnum == 1 && a:direction ==# 'up'
    normal! G
    let g:__clap_display_curlnum = lastlnum
  else
    if a:direction ==# 'down'
      normal! j
      let g:__clap_display_curlnum +=1
    else
      normal! k
      let g:__clap_display_curlnum -=1
    endif
  endif
  if !s:support_multi_selection
    call clap#sign#toggle_cursorline()
  endif
endfunction

if has('nvim')
  function! clap#handler#navigate_result(direction) abort
    call g:clap.display.goto_win()

    call s:navigate(a:direction)

    call g:clap.input.goto_win()

    call g:clap.provider.on_move()

    " Must return '' explicitly
    return ''
  endfunction
else
  function! clap#handler#navigate_result(direction) abort
    call s:navigate(a:direction)
    call g:clap.provider.on_move()
  endfunction
endif

function! clap#handler#sink() abort
  " This could be more robust by checking the exact matches count, but this should also be enough.
  if g:clap.display.get_lines() == [g:clap_no_matches_msg]
    call clap#handler#exit()
    return
  endif

  try
    if s:use_multi_selection
      let selected = clap#sign#get()
      if empty(selected)
        let curline = g:clap.display.getcurline()
        call g:clap.provider.sink(curline)
      else
        let lines = map(selected, 'getbufline(g:clap.display.bufnr, v:val)[0]')
        call g:clap.provider.sink_star(lines)
      endif
    else
      let curline = g:clap.display.getcurline()
      call g:clap.provider.sink(curline)
    endif
  catch
    call clap#error('clap#handler#sink: '.v:exception)
  finally
    call clap#handler#exit()
  endtry
endfunction

function! clap#handler#exit() abort
  let s:use_multi_selection = v:false
  let s:support_multi_selection = v:false
  call clap#exit()
endfunction

function! clap#handler#init() abort
  let s:support_multi_selection = g:clap.provider.support_multi_selection()
endfunction

function! clap#handler#select_toggle() abort
  if !s:support_multi_selection
        \ && !get(g:, 'clap_multi_selection_warning_silent', 0)
    call clap#error('<Tab> is unusable, set g:clap_multi_selection_warning_silent = 1 to suppress this warning.')
    return ''
  endif

  call clap#sign#toggle()

  let s:use_multi_selection = v:true

  return ''
endfunction

let &cpo = s:save_cpo
unlet s:save_cpo
