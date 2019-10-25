" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Neovim floating_win UI.

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

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

let s:symbol_left_bufnr = nvim_create_buf(v:false, v:true)
let s:symbol_right_bufnr = nvim_create_buf(v:false, v:true)

let s:preview_bufnr = nvim_create_buf(v:false, v:true)

let s:exists_deoplete = exists('*deoplete#custom#buffer_option')

let s:symbol_left = g:__clap_search_box_border_symbol.left
let s:symbol_right = g:__clap_search_box_border_symbol.right
let s:symbol_width = strdisplaywidth(s:symbol_right)

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

function! s:prepare_display_opts() abort
  return {
        \ 'width': &columns * 2 / 3,
        \ 'height': &lines  * 1 / 3,
        \ 'row': &lines / 3 - 1,
        \ 'col': &columns * 2 / 3 / 4,
        \ 'relative': 'editor',
        \ }
endfunction

let s:display_opts = s:prepare_display_opts()

function! clap#floating_win#reconfigure_display_opts() abort
  let s:display_opts = s:prepare_display_opts()
endfunction

let s:display_winhl = 'Normal:ClapDisplay,EndOfBuffer:ClapDisplayInvisibleEndOfBuffer,SignColumn:ClapDisplay'
let s:preview_winhl = 'Normal:ClapPreview,EndOfBuffer:ClapPreviewInvisibleEndOfBuffer,SignColumn:ClapPreview'

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
  call matchadd('ClapNoMatchesFound', g:__clap_no_matches_pattern, 10, 1001, {'window': s:display_winid})
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
        \ '&number': 0,
        \ '&cursorline': 0,
        \ '&signcolumn': 'yes',
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
  call s:try_adjust_preview()
endfunction

function! g:clap#floating_win#display.compact() abort
  let height = g:clap.display.line_count()
  let opts = nvim_win_get_config(s:display_winid)
  if opts.height != height
    let opts.height = height
    call nvim_win_set_config(s:display_winid, opts)
    call s:try_adjust_preview()
  endif
endfunction

function! s:open_win_border_left() abort
  if s:symbol_width > 0
    let opts = nvim_win_get_config(s:display_winid)
    let opts.row -= 1
    let opts.width = s:symbol_width
    let opts.height = 1
    let opts.focusable = v:false

    silent let s:symbol_left_winid = nvim_open_win(s:symbol_left_bufnr, v:false, opts)

    call setwinvar(s:symbol_left_winid, '&winhl', 'Normal:ClapSymbol')
    call setbufvar(s:symbol_left_bufnr, '&filetype', 'clap_spinner')
    call setbufvar(s:symbol_left_bufnr, '&signcolumn', 'no')

    call setbufline(s:symbol_left_bufnr, 1, s:symbol_left)
  endif
endfunction

function! g:clap#floating_win#spinner.open() abort
  let opts = nvim_win_get_config(s:display_winid)
  let opts.col += s:symbol_width
  let opts.row -= 1
  let opts.width = clap#spinner#width()
  let opts.height = 1
  let opts.focusable = v:false

  silent let s:spinner_winid = nvim_open_win(s:spinner_bufnr, v:false, opts)

  call setwinvar(s:spinner_winid, '&winhl', 'Normal:ClapSpinner')
  call setbufvar(s:spinner_bufnr, '&filetype', 'clap_spinner')
  call setbufvar(s:spinner_bufnr, '&signcolumn', 'no')

  let g:clap.spinner = get(g:clap, 'spinner', {})
  let g:clap.spinner.winid = s:spinner_winid
endfunction

function! g:clap#floating_win#input.open() abort
  let opts = nvim_win_get_config(s:spinner_winid)
  let opts.col += opts.width
  let opts.width = s:display_opts.width - opts.width - s:symbol_width * 2
  let opts.focusable = v:true

  let g:clap#floating_win#input.width = opts.width

  silent let s:input_winid = nvim_open_win(s:input_bufnr, v:true, opts)

  let w:clap_query_hi_id = matchaddpos('ClapQuery', [1])

  call setwinvar(s:input_winid, '&winhl', 'Normal:ClapInput')
  call setbufvar(s:input_bufnr, '&filetype', 'clap_input')
  let s:save_completeopt = &completeopt
  call nvim_set_option('completeopt', '')
  call setbufvar(s:input_bufnr, 'coc_suggest_disable', 1)
  if s:exists_deoplete
    call deoplete#custom#buffer_option('auto_complete', v:false)
  endif

  let g:clap.input.winid = s:input_winid
endfunction

function! s:open_win_border_right() abort
  if s:symbol_width > 0
    let opts = nvim_win_get_config(s:input_winid)
    let opts.col += opts.width
    let opts.width = s:symbol_width
    let opts.focusable = v:false

    silent let s:symbol_right_winid = nvim_open_win(s:symbol_right_bufnr, v:false, opts)

    call setwinvar(s:symbol_right_winid, '&winhl', 'Normal:ClapSymbol')
    call setbufvar(s:symbol_right_bufnr, '&filetype', 'clap_spinner')
    call setbufvar(s:symbol_right_bufnr, '&signcolumn', 'no')

    call setbufline(s:symbol_right_bufnr, 1, s:symbol_right)
  endif
endfunction

function! s:try_adjust_preview() abort
  if exists('s:preview_winid')
    let preview_opts = nvim_win_get_config(s:preview_winid)
    let opts = nvim_win_get_config(s:display_winid)
    let preview_opts.row = opts.row + opts.height
    call nvim_win_set_config(s:preview_winid, preview_opts)
  endif
endfunction

function! s:adjust_display_for_border_symbol() abort
  let opts = nvim_win_get_config(s:display_winid)
  let opts.col += s:symbol_width
  let opts.width -= s:symbol_width * 2
  call nvim_win_set_config(s:display_winid, opts)
endfunction

function! clap#floating_win#preview.show(lines) abort
  if !exists('s:preview_winid')
    let opts = nvim_win_get_config(s:display_winid)
    let opts.row += opts.height
    let opts.height = opts.height / 2

    silent let s:preview_winid = nvim_open_win(s:preview_bufnr, v:false, opts)

    call setwinvar(s:preview_winid, '&winhl', s:preview_winhl)
    " call setwinvar(s:preview_winid, '&winblend', 15)

    call clap#api#setbufvar_batch(s:preview_bufnr, {
          \ '&number': 0,
          \ '&cursorline': 0,
          \ '&signcolumn': 'no',
          \ })

    let g:clap#floating_win#preview.winid = s:preview_winid
    let g:clap#floating_win#preview.bufnr = s:preview_bufnr
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
  call s:open_win_border_left()
  call g:clap#floating_win#spinner.open()
  call g:clap#floating_win#input.open()
  call s:open_win_border_right()

  " This seemingly does not look good.
  " call s:adjust_display_for_border_symbol()

  call clap#_init()

  augroup ClapEnsureAllClosed
    autocmd!
    " autocmd BufEnter,WinEnter,WinLeave * call s:ensure_closed()
  augroup END

  call g:clap.input.goto_win()

  call g:clap.provider.on_enter()

  silent doautocmd <nomodeline> User ClapOnEnter

  startinsert

  call g:clap.provider.apply_query()
endfunction

function! s:win_close(winid) abort
  noautocmd call clap#util#nvim_win_close_safe(a:winid)
endfunction

function! clap#floating_win#close() abort
  silent! autocmd! ClapEnsureAllClosed

  if s:symbol_width > 0
    call s:win_close(s:symbol_left_winid)
    call s:win_close(s:symbol_right_winid)
  endif

  noautocmd call g:clap#floating_win#preview.close()
  call s:win_close(g:clap.input.winid)
  call s:win_close(g:clap.spinner.winid)

  " I don't know why, but this could be related to the cursor move in grep.vim
  " thus I have to go back to the start window in grep.vim
  call s:win_close(g:clap.display.winid)

  let &completeopt = s:save_completeopt
  if s:exists_deoplete
    call deoplete#custom#buffer_option('auto_complete', v:true)
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
