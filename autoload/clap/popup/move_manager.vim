" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Manage the movement and mock user input for popup.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:input = ''
let s:input_timer = -1
let s:input_delay = get(g:, 'clap_popup_input_delay', 100)
let s:cursor_shape = get(g:, 'clap_popup_cursor_shape', '|')
let s:cursor_length = strlen(s:cursor_shape)

let s:move_manager = {}

" Returns part of s:input like strcharpart(), but composing characters are not
" counted separately.
function! s:strpart_input(start, ...) abort
  if s:input ==# ''
    return ''
  endif
  let start = max([a:start, 0])
  let start_byte = byteidx(s:input, start)
  if start_byte < 0
    return ''
  endif
  let len = -1
  if a:0 >= 1
    if a:1 <= 0
      return ''
    endif
    let len = byteidx(s:input, start + a:1) - start_byte
  endif
  if len < 0
    return strpart(s:input, start_byte)
  else
    return strpart(s:input, start_byte, len)
  endif
endfunction

function! s:move_manager.ctrl_a(_winid) abort
  let s:cursor_idx = 0
  call s:mock_input()
endfunction

function! s:move_manager.ctrl_b(_winid) abort
  let s:cursor_idx -= 1
  if s:cursor_idx < 0
    let s:cursor_idx = 0
  endif
  call s:mock_input()
endfunction

function! s:move_manager.ctrl_f(_winid) abort
  let s:cursor_idx += 1
  let input_len = strchars(s:input, 1)
  if s:cursor_idx > input_len
    let s:cursor_idx = input_len
  endif
  call s:mock_input()
endfunction

function! s:move_manager.ctrl_e(_winid) abort
  let s:cursor_idx = strchars(s:input, 1)
  call s:mock_input()
endfunction

function! s:move_manager.ctrl_l(_winid) abort
  call clap#handler#relaunch_providers()
endfunction

function! s:apply_on_typed() abort
  if g:clap.provider.is_sync()
    let g:__clap_should_refilter = v:true
  endif
  call g:clap.provider.on_typed()
  call s:mock_input()
endfunction

function! s:backspace() abort
  if s:cursor_idx <= 0
    return 1
  endif
  let truncated = s:strpart_input(0, s:cursor_idx-1)
  let remained = s:strpart_input(s:cursor_idx)
  let s:input = truncated.remained
  let s:cursor_idx -= 1
  if s:cursor_idx < 0
    let s:cursor_idx = 0
  endif
endfunction

function! s:move_manager.bs(_winid) abort
  call s:backspace()
  if has_key(g:clap.provider._(), 'bs_action')
    call s:mock_input()
    call g:clap.provider._().bs_action()
  else
    call s:apply_on_typed()
  endif
endfunction

function! s:move_manager.ctrl_d(_winid) abort
  if s:cursor_idx >= strchars(s:input, 1)
    return
  endif
  let remained = s:strpart_input(0, s:cursor_idx)
  let truncated = s:strpart_input(s:cursor_idx+1)
  let s:input = remained.truncated
  call s:apply_on_typed()
endfunction

function! s:move_manager.ctrl_u(_winid) abort
  if empty(s:input)
    return 1
  endif
  let s:input = ''
  let s:cursor_idx = 0
  call s:apply_on_typed()
endfunction

" noautocmd is neccessary in that too many plugins use redir, otherwise we'll
" see E930: Cannot use :redir inside execute().
let s:move_manager["\<C-J>"] = { winid -> win_execute(winid, 'noautocmd call clap#navigation#linewise("down")') }
let s:move_manager["\<Down>"] = s:move_manager["\<C-J>"]
let s:move_manager["\<C-K>"] = { winid -> win_execute(winid, 'noautocmd call clap#navigation#linewise("up")') }
let s:move_manager["\<Up>"] = s:move_manager["\<C-K>"]
let s:move_manager["\<PageUp>"] = { winid -> win_execute(winid, 'noautocmd call clap#navigation#scroll("up")') }
let s:move_manager["\<PageDown>"] = { winid -> win_execute(winid, 'noautocmd call clap#navigation#scroll("down")') }
let s:move_manager["\<Tab>"] = { winid -> win_execute(winid, 'noautocmd call clap#handler#tab_action()') }
let s:move_manager["\<CR>"] = { _winid -> clap#handler#sink() }
let s:move_manager["\<Esc>"] = { _winid -> clap#handler#exit() }
let s:move_manager["\<C-A>"] = s:move_manager.ctrl_a
let s:move_manager["\<Home>"] = s:move_manager.ctrl_a
let s:move_manager["\<C-B>"] = s:move_manager.ctrl_b
let s:move_manager["\<Left>"] = s:move_manager.ctrl_b
let s:move_manager["\<C-F>"] = s:move_manager.ctrl_f
let s:move_manager["\<Right>"] = s:move_manager.ctrl_f
let s:move_manager["\<C-E>"] = s:move_manager.ctrl_e
let s:move_manager["\<End>"] = s:move_manager.ctrl_e
let s:move_manager["\<BS>"] = s:move_manager.bs
let s:move_manager["\<C-H>"] = s:move_manager.bs
let s:move_manager["\<C-D>"] = s:move_manager.ctrl_d
let s:move_manager["\<C-G>"] = s:move_manager["\<Esc>"]
let s:move_manager["\<C-U>"] = s:move_manager.ctrl_u
let s:move_manager["\<C-L>"] = s:move_manager.ctrl_l

function! s:define_open_action_filter() abort
  for k in keys(g:clap_open_action)
    let lhs = substitute(toupper(k), 'CTRL', 'C', '')
    execute 'let s:move_manager["\<'.lhs.'>"] = { _winid -> clap#handler#try_open("'.k.'") }'
  endfor
endfunction

call s:define_open_action_filter()

function! s:move_manager.printable(key) abort
  let s:input = s:strpart_input(0, s:cursor_idx).a:key.s:strpart_input(s:cursor_idx)
  let s:cursor_idx = strchars(s:strpart_input(0, s:cursor_idx) . a:key, 1)

  " Always hold a delay before reacting actually.
  "
  " FIXME
  " If the slow renderring of vim job is resolved, this delay could be removed.
  "
  " apply_input should happen earlier than mock_input
  " call s:apply_input('')
  "
  call s:apply_input_with_delay()

  call s:mock_input()
endfunction

function! s:apply_input(_timer) abort
  if g:clap.provider.is_pure_async()
    call g:clap.provider.jobstop()
  endif
  call g:clap.provider.on_typed()
endfunction

function! s:apply_input_with_delay() abort
  if clap#handler#relaunch_is_ok()
    return
  endif
  if s:input_timer != -1
    call timer_stop(s:input_timer)
  endif
  let s:input_timer = timer_start(s:input_delay, function('s:apply_input'))
endfunction

function! s:hl_cursor() abort
  if exists('w:clap_cursor_id')
    call matchdelete(w:clap_cursor_id)
  endif
  let w:clap_cursor_id = matchaddpos('ClapPopupCursor', [[1, byteidx(s:input, s:cursor_idx) + 1, s:cursor_length]])
endfunction

function! s:mock_input() abort
  let input = s:strpart_input(0, s:cursor_idx).s:cursor_shape.s:strpart_input(s:cursor_idx)
  let input_winid = g:clap#popup#input.winid
  call popup_settext(input_winid, input)
  call win_execute(input_winid, 'noautocmd call s:hl_cursor()')
endfunction

function! clap#popup#move_manager#mock_input() abort
  call s:mock_input()
endfunction

function! clap#popup#move_manager#filter(winid, key) abort
  try
    if has_key(s:move_manager, a:key)
      call s:move_manager[a:key](a:winid)
      return 1
    endif

    " Should catch every key.
    if a:key ==? "\<CursorHold>"
      return 1
    endif

    let char_nr = char2nr(a:key)

    " ASCII printable characters and multibyte characters
    if char_nr >= 32 && char_nr < 126 || byteidx(a:key, 1) > 1
      call s:move_manager.printable(a:key)
    endif
  catch
    let l:error_info = ['provider.on_typed:'] + split(v:throwpoint, '\[\d\+\]\zs') + [v:exception]
    call g:clap.display.set_lines(l:error_info)
    call g:clap#display_win.shrink()
    call clap#spinner#set_idle()
    return 1
  endtry

  return 1
endfunction

function! clap#popup#move_manager#init() abort
  let s:input = ''
  let s:cursor_idx = 0
endfunction

function! clap#popup#move_manager#get_input() abort
  return s:input
endfunction

function! clap#popup#move_manager#set_input(input) abort
  let s:input = a:input
  call s:mock_input()
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
