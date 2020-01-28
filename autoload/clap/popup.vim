" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Vim popup UI and interaction.

let s:save_cpo = &cpoptions
set cpoptions&vim

let g:clap#popup#preview = {}
let g:clap#popup#display = {}
let g:clap#popup#input = {}

" TODO use a flexiable width
let s:indicator_width = 18

let s:exists_deoplete = exists('*deoplete#custom#buffer_option')

let s:symbol_left = g:__clap_search_box_border_symbol.left
let s:symbol_right = g:__clap_search_box_border_symbol.right
let s:symbol_width = strdisplaywidth(s:symbol_right)

"  ----------------------------------------
" | spinner |     input        | indicator |
" |----------------------------------------|
" |              display                   |
" |----------------------------------------|
" |              preview                   |
"  ----------------------------------------
function! s:execute_in_display() abort
  let w:clap_no_matches_id = matchadd('ClapNoMatchesFound', g:__clap_no_matches_pattern)
  setlocal signcolumn=yes norelativenumber
endfunction

function! s:create_display() abort
  if !exists('s:display_winid') || empty(popup_getpos(s:display_winid))
    let s:display_opts = clap#layout#calc()

    let s:display_winid = popup_create([], {
          \ 'wrap': v:false,
          \ 'filter': function('clap#popup#move_manager#filter'),
          \ 'zindex': 1000,
          \ 'mapping': v:false,
          \ 'callback': function('s:callback'),
          \ 'scrollbar': 0,
          \ 'highlight': 'ClapDisplay',
          \ 'cursorline': 0,
          \ 'col': s:display_opts.col,
          \ 'line': s:display_opts.row,
          \ 'minwidth': s:display_opts.width,
          \ 'maxwidth': s:display_opts.width,
          \ 'maxheight': s:display_opts.height,
          \ 'minheight': s:display_opts.height,
          \ })

    let g:clap#popup#display.width = s:display_opts.width

    call win_execute(s:display_winid, 'call s:execute_in_display()')
    call popup_hide(s:display_winid)

    let g:clap.display.winid = s:display_winid
  endif
  let s:display_bufnr = winbufnr(s:display_winid)
  let g:clap.display.bufnr = s:display_bufnr
endfunction

let g:clap#popup#display.open = function('s:create_display')

function! g:clap#popup#display.shrink_if_undersize() abort
  let pos = popup_getpos(s:display_winid)
  let line_count = g:clap.display.line_count()
  if line_count < s:display_opts.height
    let pos.maxheight = line_count
    let pos.minheight = line_count
  else
    let pos.minheight = s:display_opts.height
    let pos.maxheight = s:display_opts.height
  endif
  call popup_move(s:display_winid, pos)

  call s:try_adjust_preview()
endfunction

function! g:clap#popup#display.shrink() abort
  let pos = popup_getpos(s:display_winid)
  let line_count = g:clap.display.line_count()
  if pos.height != line_count
    let pos.minheight = line_count
    let pos.maxheight = line_count
    call popup_move(s:display_winid, pos)
    call s:try_adjust_preview()
  endif
endfunction

function! s:try_adjust_preview() abort
  if exists('s:preview_winid') && !empty(popup_getpos(s:preview_winid))
    let pos = popup_getpos(s:display_winid)
    let preview_pos = popup_getpos(s:preview_winid)
    let preview_pos.line = pos.line + pos.height
    call popup_move(s:preview_winid, preview_pos)
  endif
endfunction

function! s:create_preview() abort
  if !exists('s:preview_winid') || empty(popup_getpos(s:preview_winid))
    let pos = popup_getpos(s:display_winid)
    let line = pos.line + pos.height
    let s:preview_winid = popup_create([], {
          \ 'wrap': v:false,
          \ 'zindex': 100,
          \ 'scrollbar': 0,
          \ 'highlight': 'ClapPreview',
          \ 'col': pos.col,
          \ 'line': line,
          \ 'minwidth': pos.width,
          \ 'maxwidth': pos.width,
          \ })
    call popup_hide(s:preview_winid)
    call win_execute(s:preview_winid, 'setlocal nonumber')
    let g:clap#popup#preview.winid = s:preview_winid
    let g:clap#popup#preview.bufnr = winbufnr(s:preview_winid)
  endif
endfunction

function! s:create_indicator() abort
  if !exists('s:indicator_winid') || empty(popup_getpos(s:indicator_winid))
    let pos = popup_getpos(s:display_winid)
    let pos.line = pos.line - 1
    let pos.col = pos.col + pos.width - s:indicator_width - s:symbol_width
    let pos.minwidth = s:indicator_width
    let pos.maxwidth = s:indicator_width
    let pos.highlight = 'ClapInput'
    let pos.wrap = v:false
    let pos.zindex = 100
    let s:indicator_winid = popup_create([], pos)
    call popup_hide(s:indicator_winid)
    call win_execute(s:indicator_winid, 'setlocal nonumber')
  endif
endfunction

function! s:create_symbol_right() abort
  if s:symbol_width > 0
    if !exists('s:symbol_right_winid') || empty(popup_getpos(s:symbol_right_winid))
      let pos = popup_getpos(s:display_winid)
      let pos.line = pos.line - 1
      let pos.col = pos.col + pos.width - s:symbol_width
      let pos.minwidth = s:symbol_width
      let pos.maxwidth = pos.minwidth
      let pos.highlight = 'ClapSymbol'
      let pos.wrap = v:false
      let pos.zindex = 100
      let s:symbol_right_winid = popup_create(s:symbol_right, pos)
      call popup_hide(s:symbol_right_winid)
      call win_execute(s:symbol_right_winid, 'setlocal nonumber')
    endif
  endif
endfunction

function! s:create_symbol_left() abort
  if s:symbol_width > 0
    if !exists('s:symbol_left_winid') || empty(popup_getpos(s:symbol_left_winid))
      let pos = popup_getpos(s:display_winid)
      let pos.line = pos.line - 1
      let pos.minwidth = s:symbol_width
      let pos.maxwidth = pos.minwidth
      let pos.highlight = 'ClapSymbol'
      let pos.wrap = v:false
      let pos.zindex = 100
      let s:symbol_left_winid = popup_create(s:symbol_left, pos)
      call popup_hide(s:symbol_left_winid)
      call win_execute(s:symbol_left_winid, 'setlocal nonumber')
    endif
  endif
endfunction

function! s:create_spinner() abort
  if !exists('s:spinner_winid') || empty(popup_getpos(s:spinner_winid))
    let pos = popup_getpos(s:display_winid)
    let pos.col += s:symbol_width
    let pos.line = pos.line - 1
    let pos.minwidth = clap#spinner#width() + 2
    let pos.maxwidth = pos.minwidth
    let pos.highlight = 'ClapSpinner'
    let pos.wrap = v:false
    let pos.zindex = 100
    let s:spinner_winid = popup_create(clap#spinner#get(), pos)
    call popup_hide(s:spinner_winid)
    call win_execute(s:spinner_winid, 'setlocal nonumber')
    let g:clap_spinner_winid = s:spinner_winid
  endif
endfunction

function! s:adjust_spinner() abort
  let pos = popup_getpos(s:spinner_winid)
  if empty(pos)
    return
  endif
  let spinner_width = clap#spinner#width()
  if pos.width != spinner_width
    let pos.minwidth = spinner_width
    let pos.maxwidth = spinner_width
    call popup_move(s:spinner_winid, pos)
    let input_pos = popup_getpos(s:input_winid)
    let input_pos.col = pos.col + spinner_width
    let input_pos.minwidth = s:display_opts.width - s:indicator_width - spinner_width
    let input_pos.maxwidth = input_pos.minwidth
    call popup_move(s:input_winid, input_pos)
  endif
endfunction

function! clap#popup#shrink_spinner() abort
  call s:adjust_spinner()
endfunction

function! s:execute_in_input() abort
  let s:save_completeopt = &completeopt
  set completeopt=
  setlocal nonumber
  let w:clap_search_text_hi_id = matchaddpos('ClapSearchText', [1])
  let b:coc_suggest_disable = 1
endfunction

function! s:create_input() abort
  if !exists('s:input_winid') || empty(popup_getpos(s:input_winid))
    let pos = popup_getpos(s:display_winid)
    let pos.line = pos.line - 1
    let spinner_width = clap#spinner#width()
    let pos.col += spinner_width + s:symbol_width
    let pos.minwidth = s:display_opts.width - s:indicator_width - spinner_width - s:symbol_width
    let pos.maxwidth = pos.minwidth
    let pos.highlight = 'ClapInput'
    let pos.wrap = v:false
    let pos.zindex = 100
    let s:input_winid = popup_create([], pos)
    call popup_hide(s:input_winid)

    call win_execute(s:input_winid, 'call s:execute_in_input()')
    if s:exists_deoplete
      call deoplete#custom#buffer_option('auto_complete', v:false)
    endif
    let g:clap#popup#input.winid = s:input_winid
  endif
endfunction

" Depreacted: Now we don't choose the hide way for the benefit of reusing the popup buffer,
" for it could be very problematic.
function! s:hide_all() abort
  call popup_hide(s:display_winid)
  call popup_hide(s:preview_winid)
  call popup_hide(s:indicator_winid)
  call popup_hide(s:input_winid)
  call popup_hide(s:spinner_winid)
endfunction

function! s:close_others() abort
  noautocmd call popup_close(s:preview_winid)
  noautocmd call popup_close(s:indicator_winid)
  noautocmd call popup_close(s:input_winid)
  noautocmd call popup_close(s:spinner_winid)
  if exists('s:symbol_left_winid')
    noautocmd call popup_close(s:symbol_left_winid)
  endif
  if exists('s:symbol_right_winid')
    noautocmd call popup_close(s:symbol_right_winid)
  endif
endfunction

" This somehow doesn't get called if you don't map <C-C> to <C-[>.
function! s:callback(_id, _result) abort
  unlet s:display_winid
  call clap#handler#exit()
endfunction

function! g:clap#popup#preview.show(lines) abort
  let display_pos = popup_getpos(s:display_winid)
  let col = display_pos.col
  let line = display_pos.line + display_pos.height
  let minwidth = display_pos.width
  call popup_move(s:preview_winid, {'col': col, 'line': line})

  call popup_show(s:preview_winid)
  call popup_settext(s:preview_winid, a:lines)
endfunction

function! g:clap#popup#preview.hide() abort
  if exists('s:preview_winid')
    call popup_hide(s:preview_winid)
  endif
endfunction

function! s:open_popup() abort
  call s:create_display()

  if s:symbol_width > 0
    call s:create_symbol_left()
    call s:create_symbol_right()
  endif
  call s:create_preview()
  call s:create_indicator()
  call s:create_input()
  call s:create_spinner()

  call clap#popup#move_manager#mock_input()

  call s:show_all()
endfunction

function! s:show_all() abort
  call popup_show(s:display_winid)
  call popup_show(s:indicator_winid)
  call popup_show(s:input_winid)
  call popup_show(s:spinner_winid)
  if exists('s:symbol_left_winid')
    call popup_show(s:symbol_left_winid)
  endif
  if exists('s:symbol_right_winid')
    call popup_show(s:symbol_right_winid)
  endif
endfunction

function! clap#popup#open() abort
  call clap#popup#move_manager#init()
  let g:__clap_display_curlnum = 1

  let s:save_t_ve = &t_ve
  set t_ve=

  call s:open_popup()
  call s:adjust_spinner()

  let g:clap_indicator_winid = s:indicator_winid

  call clap#_init()

  " TODO more roboust?
  augroup ClapEnsureAllClosed
    autocmd!
    autocmd BufEnter,WinEnter,WinLeave * call clap#popup#close()
  augroup END

  call g:clap.provider.try_set_syntax()
  call g:clap.provider.on_enter()

  silent doautocmd <nomodeline> User ClapOnEnter

  call g:clap.provider.apply_query()
endfunction

function! clap#popup#close() abort
  if exists('s:display_winid')
    call popup_close(s:display_winid)
  endif

  let &t_ve = s:save_t_ve
  let &completeopt = s:save_completeopt

  if s:exists_deoplete
    call deoplete#custom#buffer_option('auto_complete', v:true)
  endif

  call s:close_others()

  silent autocmd! ClapEnsureAllClosed
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
