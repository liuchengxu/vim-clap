" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Make a compatible layer for popup/floating_win APIs.

let s:save_cpo = &cpoptions
set cpoptions&vim

" Show the message in a popup at the right-top corner.
function! clap#api#popup#notify(messages, timeout) abort
  if empty(a:messages)
    return
  endif

  " Determine the popup width based on the longest message
  let max_width = max(map(copy(a:messages), 'len(v:val)')) + 2
  let height = len(a:messages)

  if has('nvim')
    " Neovim: Floating window
    let buf = nvim_create_buf(v:false, v:true)
    call nvim_buf_set_lines(buf, 0, -1, v:true, a:messages)

    let col = &columns - max_width - 1
    let row = 1 " Top-right corner

    let opts = {
          \ 'relative': 'editor',
          \ 'width': max_width,
          \ 'height': height,
          \ 'col': col,
          \ 'row': row,
          \ 'style': 'minimal',
          \ 'border': 'single',
          \ }

    let win = nvim_open_win(buf, v:false, opts)

    " Auto-close after timeout
    call timer_start(a:timeout, {-> nvim_win_close(win, v:true)})
  else
    " Vim: Popup at top-right corner
    let col = max([&columns - max_width - 1, 1])
    let popup_id = popup_create(a:messages, {
          \ 'line': 1,
          \ 'col': col,
          \ 'border': [],
          \ 'highlight': 'Normal',
          \ 'borderhighlight': ['Comment'],
          \ 'time': a:timeout
          \ })
  endif
endfunction

function! clap#api#popup#show_outline(symbols) abort
  let symbols = a:symbols

  " Determine width based on longest symbol
  let max_width = max(map(copy(symbols), 'strdisplaywidth(v:val)')) + 2
  let height = len(symbols)

  if has('nvim')
    " Neovim Floating Window
    let buf = nvim_create_buf(v:false, v:true)
    call nvim_buf_set_lines(buf, 0, -1, v:true, symbols)

    let opts = {
          \ 'relative': 'editor',
          \ 'width': max_width,
          \ 'height': height,
          \ 'col': &columns - max_width - 2,
          \ 'row': 1,
          \ 'style': 'minimal',
          \ 'border': 'rounded',
          \ }

    let g:outline_win = nvim_open_win(buf, v:false, opts)

    " Close when cursor moves
    autocmd CursorMoved,CursorMovedI * ++once call nvim_win_close(g:outline_win, v:true) | unlet! g:outline_win
  else
    " Vim Popup
    let col = max([&columns - max_width - 2, 1])
    let g:outline_popup = popup_create(symbols, {
          \ 'line': 1,
          \ 'col': col,
          \ 'border': [],
          \ 'highlight': 'Normal',
          \ 'borderhighlight': ['Comment'],
          \ })

    " Close when cursor moves
    autocmd CursorMoved,CursorMovedI * ++once call popup_close(g:outline_popup) | unlet! g:outline_win
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
