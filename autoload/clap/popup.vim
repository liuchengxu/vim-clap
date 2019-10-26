" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Vim popup UI and interaction.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:input = ''
let s:input_timer = -1
let s:input_delay = get(g:, 'clap_popup_input_delay', 200)

let g:clap#popup#preview = {}
let g:clap#popup#display = {}
let g:clap#popup#input = {}

let s:indicator_width = 10

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
function! s:prepare_display_opts() abort
  return {
      \ 'width': &columns * 2 / 3,
      \ 'height': &lines  * 1 / 3,
      \ 'row': &lines / 3 - 1,
      \ 'col': &columns * 2 / 3 / 4,
      \ }
endfunction

let s:display_opts = s:prepare_display_opts()

function! clap#popup#reconfigure_display_opts() abort
  let s:display_opts = s:prepare_display_opts()
endfunction

function! s:execute_in_display() abort
  let w:clap_no_matches_id = matchadd('ClapNoMatchesFound', g:__clap_no_matches_pattern)
  setlocal signcolumn=yes
endfunction

function! s:create_display() abort
  if !exists('s:display_winid') || empty(popup_getpos(s:display_winid))
    let col = &signcolumn ==# 'yes' ? 2 : 1
    let col += &number ? &numberwidth : 0

    let s:display_winid = popup_create([], {
          \ 'zindex': 1000,
          \ 'wrap': v:false,
          \ 'mapping': v:false,
          \ 'cursorline': 0,
          \ 'filter': function('s:popup_filter'),
          \ 'callback': function('s:callback'),
          \ 'scrollbar': 0,
          \ 'line': s:display_opts.row,
          \ 'col': s:display_opts.col,
          \ 'minwidth': s:display_opts.width,
          \ 'maxwidth': s:display_opts.width,
          \ 'maxheight': s:display_opts.height,
          \ 'minheight': s:display_opts.height,
          \ })

    let g:clap#popup#display.width = &columns * 2 / 3

    call win_execute(s:display_winid, 'call s:execute_in_display()')
    call popup_hide(s:display_winid)

    let g:clap.display.winid = s:display_winid
  endif
  let s:display_bufnr = winbufnr(s:display_winid)
  let g:clap.display.bufnr = s:display_bufnr
endfunction

let g:clap#popup#display.open = function('s:create_display')

function! g:clap#popup#display.compact_if_undersize() abort
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

function! g:clap#popup#display.compact() abort
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
    let col = pos.col
    let line = pos.line + pos.height
    let minwidth = pos.width
    let s:preview_winid = popup_create([], {
          \ 'zindex': 100,
          \ 'col': col,
          \ 'line': line,
          \ 'minwidth': minwidth,
          \ 'maxwidth': minwidth,
          \ 'wrap': v:false,
          \ 'scrollbar': 0,
          \ 'highlight': 'ClapPreview',
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

function! s:execute_in_input() abort
  let s:save_completeopt = &completeopt
  set completeopt=
  setlocal nonumber
  let w:clap_query_hi_id = matchaddpos('ClapQuery', [1])
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

function! s:mock_input() abort
  if s:input ==# ''
        \ || type(s:cursor_idx) ==# v:t_string
        \ || s:cursor_idx == strlen(s:input)
    let input = s:input.'|'
  elseif get(s:, 'insert_at_the_begin', v:false)
    let input = s:input[0].'|'.s:input[1:]
    let s:cursor_idx = 1
  elseif s:cursor_idx == 0
    let input = '|'.s:input
  else
    let input = join([s:input[:s:cursor_idx-1], s:input[s:cursor_idx :]], '|')
  endif
  call popup_settext(s:input_winid, input)
endfunction

function! clap#popup#set_input(input) abort
  let s:input = a:input
  call s:mock_input()
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

function! s:apply_input(_timer) abort
  if g:clap.provider.is_pure_async()
    call g:clap.provider.jobstop()
  endif
  call g:clap.provider.on_typed()
endfunction

function! s:apply_input_with_delay() abort
  if s:input_timer != -1
    call timer_stop(s:input_timer)
  endif
  let s:input_timer = timer_start(s:input_delay, function('s:apply_input'))
endfunction

let s:move_manager = {}

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

function! s:move_manager.ctrl_g(_winid) abort
  echom 'Unimplemented: could be used for showing some useful env info'
endfunction

function! s:move_manager.ctrl_f(_winid) abort
  let s:cursor_idx += 1
  let input_len = strlen(s:input)
  if s:cursor_idx > input_len
    let s:cursor_idx = input_len
  endif
  call s:mock_input()
endfunction

function! s:move_manager.ctrl_e(_winid) abort
  let s:cursor_idx = strlen(s:input)
  call s:mock_input()
endfunction

function! s:apply_on_typed() abort
  if g:clap.provider.is_sync()
    let g:__clap_should_refilter = v:true
  endif
  call g:clap.provider.on_typed()
  call s:mock_input()
endfunction

function! s:move_manager.bs(_winid) abort
  if empty(s:input) || s:cursor_idx == 0
    return 1
  endif
  if s:cursor_idx == 1
    let s:input = s:input[1:]
  else
    let truncated = s:input[:s:cursor_idx-2]
    let remained = s:input[s:cursor_idx :]
    let s:input = truncated.remained
  endif
  let s:cursor_idx -= 1
  if s:cursor_idx < 0
    let s:cursor_idx = 0
  endif
  call s:apply_on_typed()
endfunction

function! s:move_manager.ctrl_d(_winid) abort
  if empty(s:input) || s:cursor_idx == strlen(s:input)
    return
  endif
  if s:cursor_idx == 0
    let s:input = s:input[1:]
  else
    let remained = s:input[:s:cursor_idx-1]
    let truncated = s:input[s:cursor_idx+1:]
    let s:input = remained.truncated
  endif
  call s:apply_on_typed()
endfunction

" noautocmd is neccessary in that too many plugins use redir, otherwise we'll
" see E930: Cannot use :redir inside execute().
let s:move_manager["\<C-J>"] = { winid -> win_execute(winid, 'noautocmd call clap#handler#navigate_result("down")') }
let s:move_manager["\<Down>"] = s:move_manager["\<C-J>"]
let s:move_manager["\<C-K>"] = { winid -> win_execute(winid, 'noautocmd call clap#handler#navigate_result("up")') }
let s:move_manager["\<Up>"] = s:move_manager["\<C-K>"]
let s:move_manager["\<Tab>"] = { winid -> win_execute(winid, 'noautocmd call clap#handler#select_toggle()') }
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
let s:move_manager["\<C-G>"] = s:move_manager.ctrl_g

function! s:define_open_action_filter() abort
  for k in keys(g:clap_open_action)
    let lhs = substitute(toupper(k), 'CTRL', 'C', '')
    execute 'let s:move_manager["\<'.lhs.'>"] = { _winid -> clap#handler#try_open("'.k.'") }'
  endfor
endfunction

call s:define_open_action_filter()

function! s:move_manager.printable(key) abort
  let s:insert_at_the_begin = v:false
  if s:input ==# '' || s:cursor_idx == strlen(s:input)
    let s:input .= a:key
    let s:cursor_idx += 1
  else
    if s:cursor_idx == 0
      let s:input = a:key . s:input
      let s:insert_at_the_begin = v:true
    else
      let s:input = s:input[:s:cursor_idx-1].a:key.s:input[s:cursor_idx :]
      let s:cursor_idx += 1
    endif
  endif

  " If the privder is async, react immediately, otherwise hold a delay.
  " FIXME
  " If the slow renderring of vim job is resolved, this cuold be removed.
  if g:clap.provider.is_sync()
    " apply_input should happen earlier than mock_input
    call s:apply_input('')
  else
    call s:apply_input_with_delay()
  endif

  call s:mock_input()
endfunction

function! s:popup_filter(winid, key) abort
  if has_key(s:move_manager, a:key)
    call s:move_manager[a:key](a:winid)
    return 1
  endif

  let char_nr = char2nr(a:key)
  " ASCII printable characters
  if char_nr >= 32 && char_nr < 126
    call s:move_manager.printable(a:key)
  endif

  return 1
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

  call s:mock_input()

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

function! clap#popup#get_input() abort
  return s:input
endfunction

function! clap#popup#open() abort
  let s:input = ''
  let s:cursor_idx = 0
  let g:__clap_display_curlnum = 1

  call s:open_popup()
  call s:adjust_spinner()

  let g:clap_indicator_winid = s:indicator_winid

  call clap#_init()

  " Currently the highlight can't be local in vim.
  " Remove this once vim support win local highlight.
  redir => s:old_signcolumn
  silent hi SignColumn
  redir END

  hi! link SignColumn ClapDisplay

  " TODO more roboust?
  augroup ClapEnsureAllClosed
    autocmd!
    autocmd BufEnter,WinEnter,WinLeave * call clap#popup#close()
  augroup END

  call g:clap.provider.on_enter()

  silent doautocmd <nomodeline> User ClapOnEnter

  call g:clap.provider.apply_query()
endfunction

function! clap#popup#close() abort
  if exists('s:old_signcolumn')
    let old_signcolumn = split(s:old_signcolumn)[2:]
    try
      silent execute 'hi! SignColumn' join(old_signcolumn, ' ')
    catch
      " Ignore E416
    finally
      unlet s:old_signcolumn
    endtry
  endif
  if exists('s:display_winid')
    call popup_close(s:display_winid)
  endif
  let &completeopt = s:save_completeopt
  if s:exists_deoplete
    call deoplete#custom#buffer_option('auto_complete', v:true)
  endif
  call s:close_others()
  silent autocmd! ClapEnsureAllClosed
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
