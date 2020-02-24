" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Handle the movement.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:old_input = ''
let s:multi_select_enabled = v:false
let s:support_multi_select = v:false

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
  if g:clap.provider.is_pure_async()
    call clap#indicator#set_none()
  endif
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
  if g:clap.display.get_lines() == [g:clap_no_matches_msg]
    call s:handle_no_matches()
    return
  endif

  let selected = clap#sign#get()
  if s:multi_select_enabled && !empty(selected)
    let Sink = g:clap.provider.sink_star
    let sink_args = map(selected, 'getbufline(g:clap.display.bufnr, v:val)[0]')
  else
    let Sink = g:clap.provider.sink
    let sink_args = g:clap.display.getcurline()
  endif

  call clap#handler#internal_exit()

  try
    call Sink(sink_args)
  catch
    call clap#helper#echo_error('clap#handler#sink: '.v:exception)
  finally
    call g:clap.provider.on_exit()
    silent doautocmd <nomodeline> User ClapOnExit
  endtry
endfunction

" clap#handler#exit() = clap#handler#internal_exit() + external on_exit hook
function! clap#handler#exit() abort
  call clap#handler#internal_exit()
  call g:clap.provider.on_exit()
  silent doautocmd <nomodeline> User ClapOnExit
endfunction

function! clap#handler#internal_exit() abort
  let s:multi_select_enabled = v:false
  let s:support_multi_select = v:false
  call clap#exit()
endfunction

function! clap#handler#init() abort
  let s:support_multi_select = g:clap.provider.support_multi_select()
endfunction

function! clap#handler#tab_action() abort
  if has_key(g:clap.provider._(), 'tab_action')
    call g:clap.provider._().tab_action()
    return ''
  endif
  return clap#handler#select_toggle()
endfunction

function! clap#handler#bs_action() abort
  if has_key(g:clap.provider._(), 'bs_action')
    call g:clap.provider._().bs_action()
  else
    call nvim_feedkeys("\<BS>", 'n', v:true)
  endif
  return ''
endfunction

function! clap#handler#select_toggle() abort
  if !s:support_multi_select
        \ && !get(g:, 'clap_multi_selection_warning_silent', 0)
    call clap#helper#echo_error('<Tab> is unusable, set g:clap_multi_selection_warning_silent = 1 to suppress this warning.')
    return ''
  endif

  noautocmd call clap#sign#toggle_cursorline_multi()
  call clap#navigation#line_down()
  redraw

  let s:multi_select_enabled = v:true

  return ''
endfunction

function! clap#handler#try_open(action) abort
  if s:multi_select_enabled
        \ || !has_key(g:clap_open_action, a:action)
        \ || g:clap.display.get_lines() == [g:clap_no_matches_msg]
    return
  endif

  let Sink = g:clap.provider._().sink

  if type(Sink) == v:t_string
        \ && index(['e', 'edit', 'edit!'], Sink) != -1

    call g:clap.start.goto_win()
    let curline = g:clap.display.getcurline()
    let open = g:clap_open_action[a:action]
    execute open curline

    call clap#_exit()

  elseif g:clap.provider.support_open_action()

    let g:clap.open_action = g:clap_open_action[a:action]
    let curline = g:clap.display.getcurline()
    call g:clap.start.goto_win()
    call g:clap.provider.sink(curline)

    call remove(g:clap, 'open_action')
    call clap#_exit()
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
