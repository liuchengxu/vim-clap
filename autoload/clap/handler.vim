" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Handle the movement.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:is_nvim = has('nvim')
let s:old_input = ''

function! clap#handler#relaunch_providers() abort
  call clap#handler#exit()
  call timer_start(10, { -> clap#for('providers') })
  call g:clap.input.set('')
endfunction

function! clap#handler#relaunch_is_ok() abort
  if g:clap.input.get() ==# g:clap_providers_relaunch_code
    call clap#handler#relaunch_providers()
    return v:true
  endif
  return v:false
endfunction

function! clap#handler#on_typed() abort
  " CursorMoved event can be triggered when the floating_win
  " has been created but not visible yet.
  if s:is_nvim && !g:clap.context.visible
    return
  endif

  if clap#handler#relaunch_is_ok()
    return
  endif

  if g:clap.provider.is_rpc_type()
    call g:clap.provider.on_typed()
    return
  endif

  let l:cur_input = g:clap.input.get()
  if s:old_input == l:cur_input
    return
  elseif strlen(s:old_input) > strlen(l:cur_input)
    " If we should refilter?
    let g:__clap_should_refilter = v:true
  endif
  let s:old_input = l:cur_input
  call g:clap.provider.on_typed()
endfunction

function! s:handle_no_matches() abort
  if has_key(g:clap.provider._(), 'on_no_matches')
    let input = g:clap.input.get()
    call clap#handler#internal_exit()
    call g:clap.provider._().on_no_matches(input)
    call g:clap.provider.on_exit()
    silent doautocmd <nomodeline> User ClapOnExit
  else
    call clap#handler#exit()
  endif
endfunction

function! clap#handler#sink() abort
  " This could be more robust by checking the exact matches count, but this should also be enough.
  if empty(g:clap.display.getcurline())
        \ || g:clap.display.get_lines() == [g:clap_no_matches_msg]
    call s:handle_no_matches()
    return
  endif

  let [Sink, sink_args] = clap#selection#get_sink_or_sink_star_params()

  call clap#handler#internal_exit()

  try
    call Sink(sink_args)
  catch
    call clap#helper#echo_error('clap#handler#sink: '.v:exception.', throwpoint:'.v:throwpoint)
  finally
    call g:clap.provider.on_exit()
    silent doautocmd <nomodeline> User ClapOnExit
  endtry
endfunction

" Similiar to clap#handler#sink() but using a custom Sink function and without
" handling the no matches case.
function! clap#handler#sink_with(Fn) abort
  call clap#handler#internal_exit()
  try
    call a:Fn()
  catch
    call clap#helper#echo_error('clap#handler#sink: '.v:exception.', throwpoint:'.v:throwpoint)
  finally
    call g:clap.provider.on_exit()
    silent doautocmd <nomodeline> User ClapOnExit
  endtry
endfunction

" clap#handler#exit() = clap#handler#internal_exit() + external on_exit hook
function! clap#handler#exit() abort
  call clap#handler#internal_exit()
  call g:clap.provider.on_exit()
  let s:old_input = ''
  silent doautocmd <nomodeline> User ClapOnExit
endfunction

function! clap#handler#internal_exit() abort
  call clap#selection#reset()
  call clap#exit()
endfunction

function! clap#handler#back_action() abort
  if has_key(g:clap.provider._(), 'back_action')
    call g:clap.provider._().back_action()
    return ''
  endif
  return ''
endfunction

function! clap#handler#cr_action() abort
  if has_key(g:clap.provider._(), 'cr_action')
    call g:clap.provider._().cr_action()
    return ''
  endif
  call clap#handler#sink()
  return ''
endfunction

function! clap#handler#tab_action() abort
  if has_key(g:clap.provider._(), 'tab_action')
    call g:clap.provider._().tab_action()
    return ''
  endif
  return clap#selection#toggle()
endfunction

function! clap#handler#bs_action() abort
  if has_key(g:clap.provider._(), 'bs_action')
    call g:clap.provider._().bs_action()
  else
    call nvim_feedkeys("\<BS>", 'n', v:true)
  endif
  return ''
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
