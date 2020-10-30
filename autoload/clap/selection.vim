" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Manage user selection and the selected entries.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:multi_select_enabled = v:false
let s:support_multi_select = v:false
let s:UNUSABLE_MULTI_SELECTION = '<Tab> is unusable, set g:clap_multi_selection_warning_silent = 1 to suppress this warning.'

function! clap#selection#get_sink_or_sink_star_params() abort
  let selected = clap#sign#get()
  if s:multi_select_enabled && !empty(selected)
    let Sink = g:clap.provider.sink_star
    let sink_args = map(selected, 'clap#api#get_origin_line_at(v:val)')
  else
    let Sink = g:clap.provider.sink
    let sink_args = g:clap.display.getcurline()
  endif
  return [Sink, sink_args]
endfunction

function! clap#selection#get_action_or_action_star_params() abort
  let selected = clap#sign#get()
  if len(selected) > 1
    let Action = g:clap.provider._()['action*']
    let action_args = map(selected, 'clap#api#get_origin_line_at(v:val)')
  else
    let Action = g:clap.provider._().action
    let action_args = g:clap.display.getcurline()
  endif
  return [Action, action_args]
endfunction

function! clap#selection#init() abort
  let s:support_multi_select = g:clap.provider.support_multi_select()
endfunction

function! clap#selection#reset() abort
  let s:multi_select_enabled = v:false
  let s:support_multi_select = v:false
endfunction

function! clap#selection#toggle() abort
  if !s:support_multi_select
        \ && !g:clap_multi_selection_warning_silent
    call clap#helper#echo_error(s:UNUSABLE_MULTI_SELECTION)
    return ''
  endif

  noautocmd call clap#sign#toggle_cursorline_multi()
  call clap#navigation#line_down()
  redraw

  let s:multi_select_enabled = v:true

  return ''
endfunction

" Returns the lines that will be applied with sink action.
function! s:get_opaque_lines() abort
  let selected = clap#sign#get()
  " User can already press the Tab or not.
  if s:multi_select_enabled && !empty(selected)
    return map(copy(selected), 'clap#api#get_origin_line_at(v:val)')
  else
    return [g:clap.display.getcurline()]
  endif
endfunction

" Apply the open action specified by `g:clap_open_action` given the (selected) lines.
function! clap#selection#try_open(action) abort
  if !has_key(g:clap_open_action, a:action)
        \ || g:clap.display.get_lines() == [g:clap_no_matches_msg]
    return
  endif

  let Sink = g:clap.provider._().sink

  if type(Sink) == v:t_string
        \ && index(['e', 'edit', 'edit!'], Sink) != -1

    call g:clap.start.goto_win()
    let open_cmd = g:clap_open_action[a:action]
    for line in s:get_opaque_lines()
      execute open_cmd line
    endfor
    call clap#_exit()

  elseif g:clap.provider.support_open_action()

    call g:clap.start.goto_win()
    let g:clap.open_action = g:clap_open_action[a:action]
    for line in s:get_opaque_lines()
      call g:clap.provider.sink(line)
    endfor
    call remove(g:clap, 'open_action')
    call clap#_exit()

  endif
  silent doautocmd <nomodeline> User ClapOnExit
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
