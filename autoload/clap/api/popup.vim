" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Compatibility layer for popup/floating_win APIs.

let s:save_cpo = &cpoptions
set cpoptions&vim

" Helper: Compute maximum width for a list of messages.
function! s:get_max_width(messages) abort
  return max(map(copy(a:messages), 'strdisplaywidth(v:val)')) + 2
endfunction

" Create a floating window (Neovim) or popup (Vim) with options.
" Supported options:
"   - timeout: Auto-close timeout (ms)
"   - close_on_cursor_move: (boolean) Auto-close on cursor movement.
"   - border: Border style (default: 'single' in Neovim, none in Vim)
"   - position: "right-top" (default) or "right-bottom"
"   - row: Override computed row.
function! s:create_popup(messages, opts) abort
  let max_width = s:get_max_width(a:messages)
  let height = len(a:messages)
  let col = &columns - max_width - 1

  " Determine row position:
  if has_key(a:opts, 'row')
    let row = a:opts.row
  else
    let pos = get(a:opts, 'position', 'right-top')
    if pos ==# 'right-bottom'
      " Place popup at bottom-right (1-line margin from bottom).
      let row = &lines - height - 1
    else
      " Default: top-right.
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
    let g:_clap_popup_id = nvim_open_win(buf, v:false, win_opts)

    " Auto-close based on timeout.
    if has_key(a:opts, 'timeout')
      call timer_start(a:opts.timeout, {-> nvim_win_close(g:_clap_popup_id, v:true)})
    endif

    " Close when the cursor moves.
    if get(a:opts, 'close_on_cursor_move', 0)
      autocmd CursorMoved,CursorMovedI * ++once call nvim_win_close(g:_clap_popup_id, v:true) | unlet! g:_clap_popup_id
    endif
  else
    " Vim: Create popup.
    let g:_clap_popup_id = popup_create(a:messages, {
          \ 'line': row,
          \ 'col': col,
          \ 'border': [],
          \ 'highlight': 'Normal',
          \ 'borderhighlight': ['Comment'],
          \ })
    if has_key(a:opts, 'timeout')
      call popup_setoptions(g:_clap_popup_id, { 'time': a:opts.timeout })
    endif
    if get(a:opts, 'close_on_cursor_move', 0)
      autocmd CursorMoved,CursorMovedI * ++once call popup_close(g:_clap_popup_id) | unlet! g:_clap_popup_id
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
  let l:opts = a:0 >= 1 ? a:1 : {}
  call s:create_popup(a:messages, extend({'timeout': a:timeout}, l:opts))
endfunction

" Show document outline symbols in a popup.
" It will auto-close when the cursor moves.
" Optional extra opts can be provided to override defaults.
function! clap#api#popup#show_outline(symbols, ...) abort
  if empty(a:symbols)
    return
  endif
  let l:opts = {'border': 'rounded', 'close_on_cursor_move': 1}
  if a:0 >= 1
    let l:opts = extend(l:opts, a:1)
  endif
  call s:create_popup(a:symbols, l:opts)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo

