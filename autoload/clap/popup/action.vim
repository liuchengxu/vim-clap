" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Action dialog based on vim popup.
scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

function! s:action_filter(id, key) abort
  " Handle shortcut key
  if has_key(s:key2choice, toupper(a:key))
    call popup_close(a:id, s:key2choice[toupper(a:key)])
    return 1
  endif

  " No shortcut, pass to generic filter
  return popup_filter_menu(a:id, a:key)
endfunc

function! s:action_callback(id, result) abort
  if a:result == -1
    return
  endif
  if has_key(s:provider_action, a:result)
    call s:provider_action[a:result]()
  else
    call clap#helper#echo_error('Invalid action choice:'.a:result)
  endif
endfunction

function! s:highlight_shortcut() abort
  call map(s:key_indices, 'matchaddpos("Function", [[v:key+1, v:val+1]])')
endfunction

function! clap#popup#action#invoke() abort
  let s:provider_action = g:clap.provider._().action
  if has_key(s:provider_action, 'title')
    let title = s:provider_action['title']()
  else
    let title = 'Choose action:'
  endif
  let choices = filter(keys(s:provider_action), 'v:val !~# "title"')

  let s:key2choice = {}
  let s:key_indices = []
  for choice in choices
    let key_idx = stridx(choice, '&')
    if key_idx == -1
      call clap#helper#echo_error('choice does not contain &: '.choice)
      continue
    endif
    call add(s:key_indices, key_idx)
    let s:key2choice[choice[key_idx+1]] = choice
  endfor

  let display_menus = map(choices, "substitute(v:val, '&', '', '')")

  let dialog_winid = popup_menu(display_menus, {
      \ 'filter': function('s:action_filter'),
      \ 'callback': function('s:action_callback'),
      \ 'title': ' '.title.' ',
      \ 'zindex': 100000,
      \ 'borderchars': ['─', '│', '─', '│', '╭', '╮', '╯', '╰'],
      \ })

  call win_execute(dialog_winid, 'call s:highlight_shortcut()')
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
