" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Perform actions on entry in the result list.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:ACTIONS_TITLE_KEY = 'title'

function! s:do_actions() abort
  let provider_actions = g:clap.provider._().actions
  if has_key(provider_actions, s:ACTIONS_TITLE_KEY)
    let title = provider_actions[s:ACTIONS_TITLE_KEY]()
  else
    let title = 'Choose actions:'
  endif
  let choices = filter(keys(provider_actions), 'v:val !~# s:ACTIONS_TITLE_KEY')
  let choice_num = confirm(title, join(choices, "\n"))
  let choice = choices[choice_num-1]
  if has_key(provider_actions, choice)
    call provider_actions[choice]()
    redraw
  else
    echoerr 'Invalid action choice: '.choice
  endif
endfunction

function! clap#actions#invoke() abort
  if !has_key(g:clap.provider._(), 'actions')
    echom 'actions not implemented in provider:'.g:clap.provider.id
    return ''
  endif
  call s:do_actions()
  return ''
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
