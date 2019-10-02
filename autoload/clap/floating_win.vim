" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Neovim floating_win UI.

let s:save_cpo = &cpo
set cpo&vim

let g:clap#floating_win#input = {}
let g:clap#floating_win#display = {}
let g:clap#floating_win#spinner = {}
let g:clap#floating_win#preview = {}

let s:spinner_bufnr = nvim_create_buf(v:false, v:true)
let g:clap.spinner.bufnr = s:spinner_bufnr

let s:input_bufnr = nvim_create_buf(v:false, v:true)
let g:clap.input.bufnr = s:input_bufnr

let s:display_bufnr = nvim_create_buf(v:false, v:true)
let g:clap.display.bufnr = s:display_bufnr

let s:preview_bufnr = nvim_create_buf(v:false, v:true)

function! s:prepare_opts(row, col, width, height, ...) abort
  let base_opts = {
        \ 'row': a:row,
        \ 'col': a:col,
        \ 'width': a:width,
        \ 'height': a:height,
        \ 'relative': 'editor',
        \ }
  return extend(base_opts, get(a:000, 0, {}))
endfunction

let s:display_opts = {
      \ 'width': &columns * 2 / 3,
      \ 'height': &lines  * 1 / 3,
      \ 'row': &lines / 3 - 1,
      \ 'col': &columns * 2 / 3 / 4,
      \ 'relative': 'editor',
      \ }

let s:display_winhl = 'Normal:ClapDisplay,EndOfBuffer:ClapDisplayInvisibleEndOfBuffer,SignColumn:ClapDisplay'
let s:preview_winhl = 'Normal:ClapPreview,EndOfBuffer:ClapPreviewInvisibleEndOfBuffer:SignColumn:ClapPreview'

"  -----------------------------
" | spinner | input             |
" |-----------------------------|
" |          display            |
" |-----------------------------|
" |          preview            |
"  -----------------------------
function! g:clap#floating_win#display.open() abort
  silent let s:display_winid = nvim_open_win(s:display_bufnr, v:true, s:display_opts)

  call setwinvar(s:display_winid, '&winhl', s:display_winhl)
  call matchadd("ClapNoMatchesFound", g:__clap_no_matches_pattern, 10, 1001, {'window': s:display_winid})
  " call setwinvar(s:display_winid, '&winblend', 15)

  let g:clap.display.winid = s:display_winid

  " call setwinvar(s:display_winid, '&listchars', 'extends:•')
  "
  " \ '&listchars': 'extends:•'
  "
  " listchars would cause some troubles in some files using tab.
  " Is there a better solution?
  call g:clap.display.setbufvar_batch({
        \ '&wrap': 0,
        \ '&number': 1,
        \ '&cursorline': 1,
        \ '&signcolumn': 'no',
        \ })
endfunction

function! g:clap#floating_win#display.compact_if_undersize() abort
  let opts = nvim_win_get_config(s:display_winid)
  if g:clap.display.line_count() < s:display_opts.height
    let opts.height = g:clap.display.line_count()
  else
    let opts.height = s:display_opts.height
  endif
  call nvim_win_set_config(s:display_winid, opts)
endfunction

function! g:clap#floating_win#spinner.open() abort
  let opts = nvim_win_get_config(s:display_winid)
  let opts.row -= 1
  let opts.width = clap#spinner#width()
  let opts.height = 1
  let opts.focusable = v:false

  silent let s:spinner_winid = nvim_open_win(s:spinner_bufnr, v:true, opts)

  call setwinvar(s:spinner_winid, '&winhl', 'Normal:ClapSpinner')
  call setbufvar(s:spinner_bufnr, '&filetype', 'clap_spinner')
  call setbufvar(s:spinner_bufnr, '&signcolumn', 'no')

  let g:clap.spinner = get(g:clap, 'spinner', {})
  let g:clap.spinner.winid = s:spinner_winid
endfunction

function! g:clap#floating_win#input.open() abort
  let opts = nvim_win_get_config(s:spinner_winid)
  let opts.col += opts.width
  let opts.width = s:display_opts.width - opts.width
  let opts.focusable = v:true

  let g:clap#floating_win#input.width = opts.width

  silent let s:input_winid = nvim_open_win(s:input_bufnr, v:true, opts)

  call setwinvar(s:input_winid, '&winhl', 'Normal:ClapInput')
  call setbufvar(s:input_bufnr, '&filetype', 'clap_input')
  call setbufvar(s:input_bufnr, '&completeopt', '')

  let g:clap.input.winid = s:input_winid
endfunction

function! clap#floating_win#preview.show(lines) abort
  if !exists('s:preview_winid')
    let opts = nvim_win_get_config(s:display_winid)
    let opts.row += opts.height
    let opts.height = opts.height / 2

    silent let s:preview_winid = nvim_open_win(s:preview_bufnr, v:true, opts)

    call setwinvar(s:preview_winid, '&winhl', s:preview_winhl)
    " call setwinvar(s:preview_winid, '&winblend', 15)

    call setbufvar(s:preview_bufnr, '&number', 0)
    call setbufvar(s:preview_bufnr, '&cursorline', 0)
    call setbufvar(s:preview_bufnr, '&signcolumn', 'no')
  endif
  call clap#util#nvim_buf_set_lines(s:preview_bufnr, a:lines)
endfunction

function! clap#floating_win#preview.close() abort
  if exists('s:preview_winid')
    call clap#util#nvim_win_close_safe(s:preview_winid)
    unlet s:preview_winid
  endif
endfunction

function! s:ensure_closed() abort
  call clap#floating_win#close()
  silent! autocmd! ClapEnsureAllClosed
endfunction

function! clap#floating_win#open() abort
  let g:__clap_display_curlnum = 1

  " The order matters.
  call g:clap#floating_win#display.open()
  call g:clap#floating_win#spinner.open()
  call g:clap#floating_win#input.open()

  call clap#spinner#init()

  call g:clap.provider.init_display_win()

  let g:clap.display.initial_size = g:clap.display.line_count()

  " Set cursorline?
  " if !g:clap.display.is_empty()
    " call setbufvar(s:display_bufnr, '&cursorline', 1)
  " endif
  if g:clap.provider.support_multi_selection()
    call setbufvar(s:display_bufnr, '&signcolumn', 'yes')
  endif

  augroup ClapEnsureAllClosed
    autocmd!
    autocmd BufEnter,WinEnter,WinLeave * call s:ensure_closed()
  augroup END

  call g:clap.input.goto_win()

  call g:clap.provider.on_enter()

  silent doautocmd <nomodeline> User ClapOnEnter

  startinsert

  if has_key(g:clap.provider, 'args')
    call feedkeys(join(g:clap.provider.args, ' '))
    call clap#indicator#set_matches('')
    call g:clap.provider.on_typed()
  endif
endfunction

function! clap#floating_win#close() abort
  silent! autocmd! ClapEnsureAllClosed

  noautocmd call g:clap#floating_win#preview.close()
  noautocmd call clap#util#nvim_win_close_safe(g:clap.input.winid)
  noautocmd call clap#util#nvim_win_close_safe(g:clap.spinner.winid)

  " I don't know why, but this could be related to the cursor move in grep.vim
  " thus I have to go back to the start window in grep.vim
  noautocmd call clap#util#nvim_win_close_safe(g:clap.display.winid)
endfunction

let &cpo = s:save_cpo
unlet s:save_cpo
