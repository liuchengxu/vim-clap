" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Manage the movement and mock user input for popup.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:input = ''
let s:input_timer = -1
let s:input_delay = get(g:, 'clap_popup_input_delay', 100)
let s:cursor_shape = get(g:, 'clap_popup_cursor_shape', '')
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

function! s:move_manager.ctrl_d(_winid) abort
  if s:cursor_idx >= strchars(s:input, 1)
    return
  endif
  let remained = s:strpart_input(0, s:cursor_idx)
  let truncated = s:strpart_input(s:cursor_idx+1)
  let s:input = remained.truncated
  call s:trigger_on_typed()
endfunction

function! s:move_manager.ctrl_e(_winid) abort
  let s:cursor_idx = strchars(s:input, 1)
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

function! s:move_manager.ctrl_l(_winid) abort
  call clap#handler#handle_mapping("\<C-l\>")
endfunction

function! s:move_manager.ctrl_n(_winwid) abort
  call clap#client#notify_provider('ctrl-n')
endfunction

function! s:move_manager.ctrl_p(_winwid) abort
  call clap#client#notify_provider('ctrl-p')
endfunction

function! s:move_manager.shift_up(_winwid) abort
  call clap#client#notify_provider('shift-up')
endfunction

function! s:move_manager.shift_down(_winwid) abort
  call clap#client#notify_provider('shift-down')
endfunction

function! s:move_manager.ctrl_u(_winid) abort
  if empty(s:input)
    return 1
  endif
  let s:input = ''
  let s:cursor_idx = 0
  call s:trigger_on_typed()
endfunction

function! s:move_manager.ctrl_w(_winid) abort
  if empty(s:input)
    return 1
  endif
  let words = split(s:input, '\W\zs')
  " Remove the last word
  let new_words = words[:-2]
  if !empty(new_words)
    " Remove the trailing non-word chars
    let new_words[-1] = matchstr(new_words[-1], '\w*')
  endif
  let s:input = join(new_words, '')
  let s:cursor_idx = strchars(s:input, 1)
  call s:mock_input()
  call s:trigger_on_typed()
endfunction

function! s:move_manager.bs(_winid) abort
  let before_bs = s:strpart_input(0, s:cursor_idx).s:strpart_input(s:cursor_idx)
  " Rust backend needs to react against the value before UI changed.
  let g:__clap_popup_input_before_backspace_applied = before_bs
  call s:backspace()
  if has_key(get(g:clap.provider._(), 'mappings', {}), "<BS>")
    call s:mock_input()
    call g:clap.provider._().mappings["<BS>"]()
  else
    call s:trigger_on_typed()
  endif
endfunction

function! s:trigger_on_typed() abort
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

function! s:move_manager.linewise_scroll_down(winid) abort
  call win_execute(a:winid, 'noautocmd call clap#navigation#linewise_scroll("down")')
endfunction

function! s:move_manager.linewise_scroll_up(winid) abort
  call win_execute(a:winid, 'noautocmd call clap#navigation#linewise_scroll("up")')
endfunction

function! s:move_manager.scroll_down(winid) abort
  call win_execute(a:winid, 'noautocmd call clap#navigation#scroll("down")')
endfunction

function! s:move_manager.scroll_up(winid) abort
  call win_execute(a:winid, 'noautocmd call clap#navigation#scroll("up")')
endfunction

" noautocmd is necessary in that too many plugins use redir, otherwise we'll
" see E930: Cannot use :redir inside execute().
let s:move_manager["\<C-A>"] = s:move_manager.ctrl_a
let s:move_manager["\<C-B>"] = s:move_manager.ctrl_b
let s:move_manager["\<Del>"] = s:move_manager.ctrl_d
let s:move_manager["\<C-D>"] = s:move_manager.ctrl_d
let s:move_manager["\<C-E>"] = s:move_manager.ctrl_e
let s:move_manager["\<End>"] = s:move_manager.ctrl_e
let s:move_manager["\<C-F>"] = s:move_manager.ctrl_f
let s:move_manager["\<C-J>"] = s:move_manager.linewise_scroll_down
let s:move_manager["\<C-K>"] = s:move_manager.linewise_scroll_up
let s:move_manager["\<C-L>"] = s:move_manager.ctrl_l
let s:move_manager["\<C-N>"] = s:move_manager.ctrl_n
let s:move_manager["\<C-P>"] = s:move_manager.ctrl_p
let s:move_manager["\<C-U>"] = s:move_manager.ctrl_u
let s:move_manager["\<C-W>"] = s:move_manager.ctrl_w
let s:move_manager["\<BS>"] = s:move_manager.bs
let s:move_manager["\<C-H>"] = s:move_manager.bs
let s:move_manager["\<Esc>"] = { _winid -> clap#handler#exit() }
let s:move_manager["\<C-G>"] = s:move_manager["\<Esc>"]
let s:move_manager["\<Up>"] = s:move_manager["\<C-K>"]
let s:move_manager["\<Down>"] = s:move_manager["\<C-J>"]
let s:move_manager["\<Home>"] = s:move_manager.ctrl_a
let s:move_manager["\<Left>"] = s:move_manager.ctrl_b
let s:move_manager["\<Right>"] = s:move_manager.ctrl_f
let s:move_manager["\<Tab>"] = { winid -> win_execute(winid, 'noautocmd call clap#handler#handle_mapping("\<Tab\>")') }
let s:move_manager["\<CR>"] = { _winid -> clap#handler#handle_mapping("\<CR\>") }
let s:move_manager["\<A-U>"] = { _winid -> clap#handler#handle_mapping("\<A-U\>") }
let s:move_manager["\<S-TAB>"] = { _winid -> clap#action#invoke() }
let s:move_manager["\<S-Up>"] = s:move_manager.shift_up
let s:move_manager["\<S-Down>"] = s:move_manager.shift_down
let s:move_manager["\<PageUp>"] = s:move_manager.scroll_up
let s:move_manager["\<PageDown>"] = s:move_manager.scroll_down
let s:move_manager["\<LeftMouse>"] = s:move_manager["\<Tab>"]
let s:move_manager["\<RightMouse>"] = s:move_manager["\<Tab>"]
let s:move_manager["\<ScrollWheelUp>"] = s:move_manager.linewise_scroll_up
let s:move_manager["\<ScrollWheelDown>"] = s:move_manager.linewise_scroll_down

function! s:define_open_action_filter() abort
  for k in keys(g:clap_open_action)
    let lhs = substitute(toupper(k), 'CTRL', 'C', '')
    execute 'let s:move_manager["\<'.lhs.'>"] = { _winid -> clap#selection#try_open("'.k.'") }'
  endfor
endfunction

call s:define_open_action_filter()

if exists('g:clap_popup_move_manager')
  for [key, value] in items(g:clap_popup_move_manager)
    execute printf('let s:move_manager["%s"] = s:move_manager["%s"]', key, value)
  endfor
endif

function! clap#popup#move_manager#register(key, value) abort
  execute printf('let s:move_manager["%s"] = %s', a:key, a:value)
endfunction

function! s:move_manager.printable(key) abort
  let s:input = s:strpart_input(0, s:cursor_idx).a:key.s:strpart_input(s:cursor_idx)
  let s:cursor_idx = strchars(s:strpart_input(0, s:cursor_idx) . a:key, 1)

  " Always hold a delay before reacting actually.
  "
  " FIXME
  " If the slow rendering of vim job is resolved, this delay could be removed.
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
  if clap#handler#relaunch_is_ok(s:input)
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
  " The trailing space is for block cursor
  let input = s:strpart_input(0, s:cursor_idx).s:cursor_shape.s:strpart_input(s:cursor_idx).' '
  let input_winid = g:clap.input.winid
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
  " Move cursor to the end of line.
  let s:cursor_idx = strchars(s:input, 1)
  call s:mock_input()
endfunction

function! clap#popup#move_manager#set_input_and_react(new) abort
  call clap#popup#move_manager#set_input(a:new)
  call g:clap.provider.on_typed()
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
