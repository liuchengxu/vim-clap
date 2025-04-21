" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Compatibility layer for popup/floating_win APIs.

let s:save_cpo = &cpoptions
set cpoptions&vim

" Track active popups
let g:popup_win = {}
let g:popup_id = {}

" Helper: Compute maximum width for a list of messages.
function! s:GetMaxWidth(messages) abort
  return max(map(copy(a:messages), 'strdisplaywidth(v:val)')) + 2
endfunction

" Close existing popup if any.
function! s:CloseExistingPopup(type) abort
  if has('nvim') && has_key(g:popup_win, a:type)
    if nvim_win_is_valid(g:popup_win[a:type])
      call nvim_win_close(g:popup_win[a:type], v:true)
    endif
    unlet! g:popup_win[a:type]
  elseif has_key(g:popup_id, a:type)
    if popup_getpos(g:popup_id[a:type]) != {}
      call popup_close(g:popup_id[a:type])
    endif
    unlet! g:popup_id[a:type]
  endif
endfunction

function! s:CloseFloatingWin(type) abort
  if has_key(g:popup_win, a:type)
    call nvim_win_close(g:popup_win[a:type], v:true)
    unlet! g:popup_win[a:type]
  endif
endfunction

" Create a floating window (Neovim) or popup (Vim) with options.
" Supported options:
"   - timeout: Auto-close timeout (ms)
"   - close_on_cursor_move: (boolean) Auto-close on cursor movement.
"   - border: Border style (default: 'single' in Neovim, none in Vim)
"   - winhl: Window highlight for Neovim (default: 'NormalFloat:Normal,FloatBorder:Comment')
"   - position: "right-top" (default) or "right-bottom"
"   - row: Override computed row.
"   - type: Popup type for tracking (e.g., "notify", "outline")
function! s:CreatePopup(messages, opts) abort
  let type = get(a:opts, 'type', 'default')
  call s:CloseExistingPopup(type) " Ensure no old popups exist

  let max_width = s:GetMaxWidth(a:messages)
  if max_width > winwidth('') / 2
    let max_width = winwidth('') / 2
  endif
  let height = len(a:messages)
  let col = &columns - max_width - 1

  " Determine row position:
  if has_key(a:opts, 'row')
    let row = a:opts.row
  else
    let pos = get(a:opts, 'position', 'right-top')
    if pos ==# 'right-bottom'
      let row = &lines - height - 1
    else
      let row = 1
    endif
  endif

  if has('nvim')
    " Neovim: Create floating window.
    let buf = nvim_create_buf(v:false, v:true)
    call nvim_buf_set_lines(buf, 0, -1, v:true, a:messages)

    let win_opts = {
          \ 'relative': 'editor',
          \ 'width': max_width,
          \ 'height': height,
          \ 'col': col,
          \ 'row': row,
          \ 'style': 'minimal',
          \ 'border': get(a:opts, 'border', 'single'),
          \ }
    let g:popup_win[type] = nvim_open_win(buf, v:false, win_opts)

    " Auto-close based on timeout.
    if has_key(a:opts, 'timeout')
      call timer_start(a:opts.timeout, { -> s:CloseFloatingWin(type)})
    endif

    " Close when the cursor moves.
    if get(a:opts, 'close_on_cursor_move', 0)
      let g:__clap_popup_type = type
      autocmd CursorMoved,CursorMovedI * ++once call s:CloseFloatingWin(g:__clap_popup_type)
    endif
  else
    " Vim: Create popup.
    let g:popup_id[type] = popup_create(a:messages, {
          \ 'line': row,
          \ 'col': col,
          \ 'border': [],
          \ 'highlight': 'Normal',
          \ 'borderhighlight': ['Comment'],
          \ })

    if has_key(a:opts, 'timeout')
      call popup_setoptions(g:popup_id[type], { 'time': a:opts.timeout })
    endif

    if get(a:opts, 'close_on_cursor_move', 0)
      let g:__clap_popup_type = type
      autocmd CursorMoved,CursorMovedI * ++once call popup_close(g:popup_id[g:__clap_popup_type]) | unlet! g:popup_id[g:__clap_popup_type]
    endif
  endif
endfunction

" Show a notification popup at the right-top or right-bottom corner.
" Usage:
"   :call clap#api#popup#notify(["Message"], 2000)
"   :call clap#api#popup#notify(["Message"], 2000, {'position': 'right-bottom'})
function! clap#api#popup#notify(messages, timeout, ...) abort
  if empty(a:messages)
    return
  endif
  let l:opts = {'type': 'notify', 'timeout': a:timeout}
  if a:0 >= 1
    let l:opts = extend(l:opts, a:1)
  endif
  call s:CreatePopup(a:messages, l:opts)
endfunction

" Show document outline symbols in a popup.
" It will auto-close when the cursor moves.
" Optional extra opts can be provided to override defaults.
function! clap#api#popup#show_outline(symbols, ...) abort
  if empty(a:symbols)
    return
  endif
  let l:opts = {'type': 'outline', 'border': 'rounded', 'close_on_cursor_move': 1}
  if a:0 >= 1
    let l:opts = extend(l:opts, a:1)
  endif
  call s:CreatePopup(a:symbols, l:opts)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
