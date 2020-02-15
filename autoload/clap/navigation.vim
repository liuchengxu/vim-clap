" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Navigate between the result list.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:lazy_load_size = 50

function! s:load_cache() abort
  let cache = g:clap.display.cache
  if len(cache) <= s:lazy_load_size
    let to_append = cache
    let g:clap.display.cache = []
  else
    let to_append = cache[:s:lazy_load_size-1]
    let g:clap.display.cache = cache[s:lazy_load_size :]
  endif
  if has_key(g:clap.provider._(), 'converter')
    let to_append = map(to_append, 'g:clap.provider._().converter(v:val)')
  endif
  " The buffer is not empty, qed.
  call g:clap.display.append_lines_uncheck(to_append)
endfunction

function! s:scroll(direction) abort
  let scroll_lines = getwinvar(g:clap.display.winid, '&scroll')
  if a:direction ==# 'down'
    execute 'normal!' scroll_lines.'j'
  elseif a:direction ==# 'top'
    normal! gg
  elseif a:direction ==# 'bottom'
    normal! G
  else
    execute 'normal!' scroll_lines.'k'
  endif

  let g:__clap_display_curlnum = line('.')
  call clap#sign#toggle_cursorline()
endfunction

function! s:navigate(direction) abort
  let curlnum = line('.')
  let lastlnum = line('$')

  if curlnum == lastlnum && a:direction ==# 'down'
    " Lazy append!
    " Append a few more lines from the cache when reaching the end of the
    " buffer.
    if empty(g:clap.display.cache)
          \ || get(g:, '__clap_do_not_use_cache', v:false)

      if !g:clap_disable_bottom_top
        normal! 1gg
      endif
    else
      call s:load_cache()
      normal! j
    endif

  elseif curlnum == 1 && a:direction ==# 'up'

    if !g:clap_disable_bottom_top
      normal! G
    endif

  else

    if a:direction ==# 'down'
      normal! j
    else
      normal! k
    endif

  endif

  let g:__clap_display_curlnum = line('.')
  call clap#sign#toggle_cursorline()
endfunction

function! s:on_move_safe() abort
  " try
    call g:clap.provider.on_move()
  " catch
    " call g:clap.preview.show([v:exception])
  " endtry
endfunction

if has('nvim')
  function! s:wrap_move(Move, args) abort
    call g:clap.display.goto_win()

    call call(a:Move, a:args)

    call g:clap.input.goto_win()
    call s:on_move_safe()

    " Must return '' explicitly
    return ''
  endfunction

  function! clap#navigation#linewise(direction) abort
    return s:wrap_move(function('s:navigate'), [a:direction])
  endfunction

  function! clap#navigation#scroll(direction) abort
    return s:wrap_move(function('s:scroll'), [a:direction])
  endfunction

  function! clap#navigation#line_down() abort
    call g:clap.display.goto_win()
    call s:navigate('down')
    call g:clap.input.goto_win()
  endfunction

else
  function! clap#navigation#linewise(direction) abort
    call s:navigate(a:direction)
    " redraw is neccessary!
    redraw
    call s:on_move_safe()
  endfunction

  function! clap#navigation#scroll(direction) abort
    call win_execute(g:clap.display.winid, 'call s:scroll(a:direction)')
  endfunction

  function! clap#navigation#line_down() abort
    call win_execute(g:clap.display.winid, 'call s:navigate("down")')
  endfunction
endif

let &cpoptions = s:save_cpo
unlet s:save_cpo
