" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Neovim floating_win UI.

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

let g:clap#floating_win#input = {}
let g:clap#floating_win#display = {}
let g:clap#floating_win#spinner = {}
let g:clap#floating_win#preview = {}

let s:has_nvim_0_5 = has('nvim-0.5')

let s:shadow_bufnr = nvim_create_buf(v:false, v:true)

let s:spinner_bufnr = nvim_create_buf(v:false, v:true)
let g:clap.spinner.bufnr = s:spinner_bufnr

let s:input_bufnr = nvim_create_buf(v:false, v:true)
let g:clap.input.bufnr = s:input_bufnr

let s:display_bufnr = nvim_create_buf(v:false, v:true)
let g:clap.display.bufnr = s:display_bufnr

let s:symbol_left_bufnr = nvim_create_buf(v:false, v:true)
let s:symbol_right_bufnr = nvim_create_buf(v:false, v:true)

let s:preview_bufnr = nvim_create_buf(v:false, v:true)

let s:indicator_bufnr = nvim_create_buf(v:false, v:true)
let g:__clap_indicator_bufnr = s:indicator_bufnr

let s:exists_deoplete = exists('*deoplete#custom#buffer_option')

let s:symbol_left = g:__clap_search_box_border_symbol.left
let s:symbol_right = g:__clap_search_box_border_symbol.right
let s:symbol_width = strdisplaywidth(s:symbol_right)

let s:shadow_winhl = 'Normal:ClapShadow,NormalNC:ClapShadow,EndOfBuffer:ClapShadow'
let s:display_winhl = 'Normal:ClapDisplay,EndOfBuffer:ClapDisplayInvisibleEndOfBuffer,SignColumn:ClapDisplay,ColorColumn:ClapDisplay'
let s:preview_winhl = 'Normal:ClapPreview,EndOfBuffer:ClapPreviewInvisibleEndOfBuffer,SignColumn:ClapPreview,ColorColumn:ClapPreview'

" shadow
"  -----------------------------
" | spinner | input             |
" |-----------------------------|
" |          display            |
" |-----------------------------|
" |          preview            |
"  -----------------------------
function! g:clap#floating_win#display.open() abort
  if exists('s:display_winid') && nvim_win_is_valid(s:display_winid)
    return
  endif
  " Check if the buffer is still valid as when switching between the sessions, it could become invalid.
  if !nvim_buf_is_valid(s:display_bufnr)
    let s:display_bufnr = nvim_create_buf(v:false, v:true)
    let g:clap.display.bufnr = s:display_bufnr
  endif

  let s:display_opts = clap#layout#calc()
  silent let s:display_winid = nvim_open_win(s:display_bufnr, v:true, s:display_opts)

  call setwinvar(s:display_winid, '&winhl', s:display_winhl)
  call setwinvar(s:display_winid, '&spell', 0)
  call matchadd('ClapNoMatchesFound', g:__clap_no_matches_pattern, 10, 1001, {'window': s:display_winid})
  " call setwinvar(s:display_winid, '&winblend', 15)

  let g:clap.display.winid = s:display_winid

  " call setwinvar(s:display_winid, '&listchars', 'extends:•')
  " \ '&listchars': 'extends:•'
  " listchars would cause some troubles in some files using tab.
  " Is there a better solution?

  call g:clap.display.setbufvar_batch({
        \ '&wrap': 0,
        \ '&number': 0,
        \ '&relativenumber': 0,
        \ '&cursorline': 0,
        \ '&signcolumn': 'yes',
        \ '&foldcolumn': 0,
        \ 'autopairs_enabled': 0,
        \ 'ale_enabled': 0,
        \ })
endfunction

function! g:clap#floating_win#display.shrink_if_undersize() abort
  if !clap#preview#is_always_open()
    let opts = nvim_win_get_config(s:display_winid)
    if g:clap.display.line_count() < s:display_opts.height
      let opts.height = g:clap.display.line_count()
    else
      let opts.height = s:display_opts.height
    endif
    call nvim_win_set_config(s:display_winid, opts)
    call s:try_adjust_preview()
  endif
endfunction

function! g:clap#floating_win#display.shrink() abort
  if !clap#preview#is_always_open()
    let height = g:clap.display.line_count()
    let opts = nvim_win_get_config(s:display_winid)
    if opts.height != height
      let opts.height = height
      call nvim_win_set_config(s:display_winid, opts)
      call s:try_adjust_preview()
    endif
  endif
endfunction

function! s:set_minimal_buf_style(bufnr, filetype) abort
  call setbufvar(a:bufnr, '&filetype', a:filetype)
  call setbufvar(a:bufnr, '&signcolumn', 'no')
  call setbufvar(a:bufnr, '&foldcolumn', 0)
endfunction

function! s:get_config_border_left() abort
  let opts = nvim_win_get_config(s:display_winid)
  let opts.row -= 1
  let opts.width = s:symbol_width
  let opts.height = 1
  let opts.focusable = v:false
  if s:has_nvim_0_5
    let opts.zindex = 1000
  endif
  return opts
endfunction

function! s:open_win_border_left() abort
  if s:symbol_width > 0
    if !nvim_buf_is_valid(s:symbol_left_bufnr)
      let s:symbol_left_bufnr = nvim_create_buf(v:false, v:true)
    endif
    silent let s:symbol_left_winid = nvim_open_win(s:symbol_left_bufnr, v:false, s:get_config_border_left())
    call setwinvar(s:symbol_left_winid, '&winhl', 'Normal:ClapSymbol')
    call s:set_minimal_buf_style(s:symbol_left_bufnr, 'clap_spinner')
    call setbufline(s:symbol_left_bufnr, 1, s:symbol_left)
  endif
endfunction

function! s:get_config_spinner() abort
  let opts = nvim_win_get_config(s:display_winid)
  let opts.col += s:symbol_width
  let opts.row -= 1
  let opts.width = clap#spinner#width()
  let opts.height = 1
  let opts.focusable = v:false
  if s:has_nvim_0_5
    let opts.zindex = 1000
  endif
  return opts
endfunction

function! g:clap#floating_win#spinner.open() abort
  if exists('s:spinner_winid') && nvim_win_is_valid(s:spinner_winid)
    return
  endif
  if !nvim_buf_is_valid(s:spinner_bufnr)
    let s:spinner_bufnr = nvim_create_buf(v:false, v:true)
    let g:clap.spinner.bufnr = s:spinner_bufnr
  endif
  silent let s:spinner_winid = nvim_open_win(s:spinner_bufnr, v:false, s:get_config_spinner())

  call setwinvar(s:spinner_winid, '&winhl', 'Normal:ClapSpinner')
  call s:set_minimal_buf_style(s:spinner_bufnr, 'clap_spinner')

  let g:clap.spinner = get(g:clap, 'spinner', {})
  let g:clap.spinner.winid = s:spinner_winid
endfunction

function! g:clap#floating_win#spinner.shrink() abort
  if exists('s:spinner_winid') && nvim_win_is_valid(s:spinner_winid)
    let width = clap#spinner#width()
    let opts = nvim_win_get_config(s:spinner_winid)
    if opts.width != width
      let opts.width = width
      call nvim_win_set_config(s:spinner_winid, opts)

      let opts = nvim_win_get_config(s:spinner_winid)
      let opts.col += opts.width
      let opts.width = s:display_opts.width - opts.width - s:symbol_width * 2 - s:indicator_width
      if opts.width < 0
        let opts.width = 1
      endif
      let g:clap#floating_win#input.width = opts.width
      call nvim_win_set_config(s:input_winid, opts)
    endif
  endif
endfunction

function! s:get_config_input() abort
  let opts = nvim_win_get_config(s:spinner_winid)
  let opts.col += opts.width
  let opts.width = s:display_opts.width - opts.width - s:symbol_width * 2 - s:indicator_width
  " E5555: API call: 'width' key must be a positive Integer
  " Avoid E5555 here and it seems to be fine later.
  if opts.width < 0
    let opts.width = 1
  endif
  let opts.focusable = v:true
  if s:has_nvim_0_5
    let opts.zindex = 1000
  endif
  return opts
endfunction

function! g:clap#floating_win#input.open() abort
  if exists('s:input_winid') && nvim_win_is_valid(s:input_winid)
    return
  endif
  let opts = s:get_config_input()
  let g:clap#floating_win#input.width = opts.width

  if !nvim_buf_is_valid(s:input_bufnr)
    let s:input_bufnr = nvim_create_buf(v:false, v:true)
    let g:clap.input.bufnr = s:input_bufnr
  endif
  silent let s:input_winid = nvim_open_win(s:input_bufnr, v:true, opts)

  let w:clap_search_text_hi_id = matchaddpos('ClapSearchText', [1])

  call setwinvar(s:input_winid, '&winhl', 'Normal:ClapInput')
  call s:set_minimal_buf_style(s:input_bufnr, 'clap_input')
  " Disable the auto-completion plugin
  let s:save_completeopt = &completeopt
  call nvim_set_option('completeopt', '')
  if s:exists_deoplete
    call deoplete#custom#buffer_option('auto_complete', v:false)
  endif
  call setwinvar(s:input_winid, 'airline_disable_statusline', 1)
  call setbufvar(s:input_bufnr, 'coc_suggest_disable', 1)
  " Disable the auto-pairs plugin
  call setbufvar(s:input_bufnr, 'coc_pairs_disabled', ['"', "'", '(', ')', '<', '>', '[', ']', '{', '}', '`'])
  call setbufvar(s:input_bufnr, 'autopairs_loaded', 1)
  call setbufvar(s:input_bufnr, 'autopairs_enabled', 0)
  call setbufvar(s:input_bufnr, 'pear_tree_enabled', 0)
  call setwinvar(s:input_winid, '&spell', 0)
  let g:clap.input.winid = s:input_winid
endfunction

function! s:get_config_shadow() abort
  let opts =  {
  \ 'relative': 'editor',
  \ 'style': 'minimal',
  \ 'width': &columns,
  \ 'height': &lines,
  \ 'row': 0,
  \ 'col': 0,
  \ }
  return opts
endfunction

function! s:open_shadow_win() abort
  if exists('s:shadow_winid') && nvim_win_is_valid(s:shadow_winid)
    return
  endif
  if !nvim_buf_is_valid(s:shadow_bufnr)
    let s:shadow_bufnr = nvim_create_buf(v:false, v:true)
  endif
  silent let s:shadow_winid = nvim_open_win(s:shadow_bufnr, v:true, s:get_config_shadow())
  call setwinvar(s:shadow_winid, '&winhl', s:shadow_winhl)
  call setwinvar(s:shadow_winid, '&winblend', g:clap_background_shadow_blend)
endfunction

function! s:get_config_indicator() abort
  let opts = nvim_win_get_config(s:input_winid)
  let opts.col += opts.width
  let opts.width = s:indicator_width
  let opts.focusable = v:false
  let opts.style = 'minimal'
  if s:has_nvim_0_5
    let opts.zindex = 1000
  endif
  return opts
endfunction

function! s:open_indicator_win() abort
  if exists('s:indicator_winid') && nvim_win_is_valid(s:indicator_winid)
    return
  endif
  if !nvim_buf_is_valid(s:indicator_bufnr)
    let s:indicator_bufnr = nvim_create_buf(v:false, v:true)
    let g:__clap_indicator_bufnr = s:indicator_bufnr
  endif
  silent let s:indicator_winid = nvim_open_win(s:indicator_bufnr, v:true, s:get_config_indicator())
  call setwinvar(s:indicator_winid, '&winhl', 'Normal:ClapIndicator')
  call setbufvar(s:indicator_bufnr, '&signcolumn', 'no')
endfunction

function! s:get_config_border_right() abort
  let opts = nvim_win_get_config(s:indicator_winid)
  let opts.col += opts.width
  let opts.width = s:symbol_width
  let opts.focusable = v:false
  if s:has_nvim_0_5
    let opts.zindex = 1000
  endif
  return opts
endfunction

function! s:open_win_border_right() abort
  if s:symbol_width > 0
    if !nvim_buf_is_valid(s:symbol_right_bufnr)
      let s:symbol_right_bufnr = nvim_create_buf(v:false, v:true)
    endif
    silent let s:symbol_right_winid = nvim_open_win(s:symbol_right_bufnr, v:false, s:get_config_border_right())
    call setwinvar(s:symbol_right_winid, '&winhl', 'Normal:ClapSymbol')
    call s:set_minimal_buf_style(s:symbol_right_bufnr, 'clap_spinner')
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

function! s:get_config_preview(height) abort
  let preview_direction = clap#preview#direction() 
  if preview_direction ==# 'LR'
    let opts = nvim_win_get_config(s:display_winid)
    let opts.row -= 1
    let opts.col += opts.width
    let opts.height += 1
  else " preview_direction ==# 'UD'
    let opts = nvim_win_get_config(s:display_winid)
    let opts.row += opts.height
    let opts.height = a:height
  endif
  let opts.style = 'minimal'

  if s:has_nvim_0_5 && g:clap_popup_border !=? 'nil'
    let opts.border = g:clap_popup_border
    if preview_direction ==# 'UD'
      let opts.width -= 2
    else " preview_direction ==# 'UD'
      let opts.height -= 2
    endif
  endif
  return opts
endfunction

function! s:create_preview_win(height) abort
  if !exists('s:display_winid') || !nvim_win_is_valid(s:display_winid)
    return
  endif

  if !nvim_buf_is_valid(s:preview_bufnr)
    let s:preview_bufnr = nvim_create_buf(v:false, v:true)
  endif
  silent let s:preview_winid = nvim_open_win(s:preview_bufnr, v:false, s:get_config_preview(a:height))

  call setwinvar(s:preview_winid, '&spell', 0)
  call setwinvar(s:preview_winid, '&winhl', s:preview_winhl)
  " call setwinvar(s:preview_winid, '&winblend', 15)

  call setbufvar(s:preview_bufnr, '&signcolumn', 'no')

  let g:clap#floating_win#preview.winid = s:preview_winid
  let g:clap#floating_win#preview.bufnr = s:preview_bufnr
endfunction

function! s:max_preview_size() abort
  if clap#preview#direction() ==# 'LR'
    return s:display_opts.height
  else
    let max_size = &lines - s:display_opts.row - s:display_opts.height - &cmdheight
    return float2nr(max_size)
  endif
endfunction

function! clap#floating_win#preview.show(lines) abort
  if !clap#preview#is_enabled()
    return
  endif

  let max_size = s:max_preview_size()
  if max_size <= 0
    call g:clap#floating_win#preview.close()
    return
  endif
  let lines = a:lines[:max_size]
  let height = len(lines)
  if !exists('s:preview_winid')
    call s:create_preview_win(height)
  else
    if clap#preview#direction() !=# 'LR'
      let opts = nvim_win_get_config(s:preview_winid)
      if opts.height != height
        let opts.height = height
        call nvim_win_set_config(s:preview_winid, opts)
      endif
    endif
  endif
  call clap#util#nvim_buf_set_lines(s:preview_bufnr, lines)
endfunction

function! clap#floating_win#preview.close() abort
  if exists('s:preview_winid')
    call clap#util#nvim_win_close_safe(s:preview_winid)
    unlet s:preview_winid
  endif
endfunction

function! clap#floating_win#preview.hide() abort
  if !clap#preview#is_always_open()
    call g:clap#floating_win#preview.close()
  endif
endfunction

function! clap#floating_win#preview.clear() abort
  call clap#util#nvim_buf_clear(s:preview_bufnr)
endfunction

function! s:ensure_closed() abort
  call clap#floating_win#close()
  silent! autocmd! ClapEnsureAllClosed
endfunction

function! s:open_shadow_before_indicator_win(yes) abort
  if a:yes
    if g:clap_enable_background_shadow
      call s:open_shadow_win()
    end
    call s:open_indicator_win()
  else
    call s:open_indicator_win()
    if g:clap_enable_background_shadow
      call s:open_shadow_win()
    end
  endif
endfunction

function! clap#floating_win#open() abort
  let g:__clap_display_curlnum = 1

  let s:save_winheight = &winheight
  let &winheight = 1

  let s:indicator_width = clap#layout#indicator_width()

  " The order matters.
  call g:clap#floating_win#display.open()
  call s:open_win_border_left()
  call g:clap#floating_win#spinner.open()
  call g:clap#floating_win#input.open()
  if clap#preview#is_enabled() && clap#preview#direction() ==# 'LR'
    call s:create_preview_win(s:display_opts.height)
  endif

  if g:clap_search_box_border_style ==# 'curve'
    let open_shadow_first = v:false
  elseif g:clap_search_box_border_style ==# 'nil'
    let open_shadow_first = v:true
  else
    let open_shadow_first = v:false
  endif
  " This tricky issue has been resolved with the newly added zindex in neovim.
  "
  " Indicator win must be opened before shadow win in some cases.
  " ref https://github.com/liuchengxu/vim-clap/issues/567#issuecomment-717554261
  call s:open_shadow_before_indicator_win(open_shadow_first)

  call s:open_win_border_right()

  " This seemingly does not look good.
  " call s:adjust_display_for_border_symbol()

  call clap#_init()

  augroup ClapEnsureAllClosed
    autocmd!
    " autocmd BufEnter,WinEnter,WinLeave * call s:ensure_closed()
  augroup END

  call g:clap.input.goto_win()

  call g:clap.provider.try_set_syntax()
  call g:clap.provider.on_enter()

  silent doautocmd <nomodeline> User ClapOnEnter

  startinsert

  let g:clap.context.visible = v:true

  call g:clap.provider.apply_query()
endfunction

function! s:win_close(winid) abort
  " Removed `noautocmd`, some user-defined autocmd might be interrupted. Ref #472
  " The interesting thing is Vim's popup won't be affected.
  call clap#util#nvim_win_close_safe(a:winid)
endfunction

function! clap#floating_win#close() abort
  let &winheight = s:save_winheight
  silent! autocmd! ClapEnsureAllClosed

  if s:symbol_width > 0
    call s:win_close(s:symbol_left_winid)
    call s:win_close(s:symbol_right_winid)
  endif

  if exists('s:shadow_winid')
    call s:win_close(s:shadow_winid)
  endif
  noautocmd call g:clap#floating_win#preview.close()
  call s:win_close(g:clap.input.winid)
  call s:win_close(g:clap.spinner.winid)
  call s:win_close(s:indicator_winid)

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
