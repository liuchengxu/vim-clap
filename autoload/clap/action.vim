" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Perform provider action against current entry

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:ACTIONS_TITLE_KEY = 'title'

" `confirm()` based action menu, this is deprecated now.
function! s:invoke_action() abort
  let provider_action = g:clap.provider._().action
  if has_key(provider_action, s:ACTIONS_TITLE_KEY)
    let title = provider_action[s:ACTIONS_TITLE_KEY]()
  else
    let title = 'Choose action:'
  endif
  let choices = filter(keys(provider_action), 'v:val !~# s:ACTIONS_TITLE_KEY')
  let choice_num = confirm(title, join(choices, "\n"))
  " User aborts the dialog
  if choice_num == 0
    return
  endif
  let choice = choices[choice_num-1]
  if has_key(provider_action, choice)
    " TODO: add `action*` for performing actions against multi-selected entries?
    call provider_action[choice]()
  else
    call clap#helper#echo_error('Invalid action choice: '.choice)
  endif
endfunction

function! clap#action#invoke() abort
  if !has_key(g:clap.provider._(), 'action')
    call clap#helper#echo_warn('action not implemented in provider '.g:clap.provider.id)
    return ''
  endif
  if has('nvim')
    call clap#floating_win#action#create()
  else
    call clap#popup#action#invoke()
  endif
  return ''
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
