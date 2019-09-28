" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Vim popup UI and interaction.

let s:input = ''
let s:input_timer = -1

let g:clap#popup#preview = {}
let g:clap#popup#display = {}
let g:clap#popup#input = {}

let s:indicator_width = 10

"  ----------------------------------------
" | spinner |     input        | indicator |
" |----------------------------------------|
" |              display                   |
" |----------------------------------------|
" |              preview                   |
"  ----------------------------------------
let s:display_opts = {
      \ 'width': &columns * 2 / 3,
      \ 'height': &lines  * 1 / 3,
      \ 'row': &lines / 3 - 1,
      \ 'col': &columns * 2 / 3 / 4,
      \ }

function! s:create_display() abort
  if !exists('s:display_winid') || empty(popup_getpos(s:display_winid))
    let col = &signcolumn ==# 'yes' ? 2 : 1
    let col += &number ? &numberwidth : 0

    let s:display_winid = popup_create([], #{
          \ wrap: v:false,
          \ mapping: v:false,
          \ cursorline: 0,
          \ filter: function('s:popup_filter'),
          \ callback: function('s:callback'),
          \ scrollbar: 0,
          \ line: s:display_opts.row,
          \ col: s:display_opts.col,
          \ minwidth: s:display_opts.width,
          \ maxwidth: s:display_opts.width,
          \ maxheight: s:display_opts.height,
          \ minheight: s:display_opts.height,
          \ })

    let g:clap#popup#display.width = &columns * 2 / 3

    call popup_hide(s:display_winid)
    " call win_execute(s:display_winid, 'setlocal nonumber')

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
endfunction

function! s:create_preview() abort
  if !exists('s:preview_winid') || empty(popup_getpos(s:preview_winid))
    let pos = popup_getpos(s:display_winid)
    let col = pos.col
    let line = pos.line + pos.height
    let minwidth = pos.width
    " If the preview win has border, then minwidth - 2.
    let s:preview_winid = popup_create([], #{
          \ col: col,
          \ line: line,
          \ minwidth: minwidth - 2,
          \ maxwidth: minwidth - 2,
          \ wrap: v:false,
          \ scrollbar: 0,
          \ border: [1, 1, 1, 1],
          \ highlight: 'ClapPreview',
          \ })
    call popup_hide(s:preview_winid)
    call win_execute(s:preview_winid, 'setlocal nonumber')
  endif
endfunction

function! s:create_indicator() abort
  if !exists('s:indicator_winid') || empty(popup_getpos(s:indicator_winid))
    let pos = popup_getpos(s:display_winid)
    let pos.line = pos.line - 1
    let pos.col = pos.col + pos.width - s:indicator_width
    let pos.minwidth = s:indicator_width
    let pos.maxwidth = s:indicator_width
    let pos.highlight = 'ClapInput'
    let pos.wrap = v:false
    let s:indicator_winid = popup_create([], pos)
    call popup_hide(s:indicator_winid)
    call win_execute(s:indicator_winid, 'setlocal nonumber')
  endif
endfunction

function! s:create_spinner() abort
  if !exists('s:spinner_winid') || empty(popup_getpos(s:spinner_winid))
    let pos = popup_getpos(s:display_winid)
    let pos.line = pos.line - 1
    " FIXME adjust the spinner size on provider open
    let pos.minwidth = clap#spinner#width() + 2
    let pos.maxwidth = pos.minwidth
    let pos.highlight = 'ClapSpinner'
    let pos.wrap = v:false
    let s:spinner_winid = popup_create([], pos)
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

function! s:create_input() abort
  if !exists('s:input_winid') || empty(popup_getpos(s:input_winid))
    let pos = popup_getpos(s:display_winid)
    let pos.line = pos.line - 1
    let spinner_width = clap#spinner#width()
    let pos.col += spinner_width
    let pos.minwidth = s:display_opts.width - s:indicator_width - spinner_width
    let pos.maxwidth = pos.minwidth
    let pos.highlight = 'ClapInput'
    let pos.wrap = v:false
    let s:input_winid = popup_create([], pos)
    call popup_hide(s:input_winid)
    call win_execute(s:input_winid, 'setlocal nonumber')
    let g:clap#popup#input.winid = s:input_winid
  endif
endfunction

function! s:hide_all() abort
  call popup_hide(s:display_winid)
  call popup_hide(s:preview_winid)
  call popup_hide(s:indicator_winid)
  call popup_hide(s:input_winid)
  call popup_hide(s:spinner_winid)
endfunction

" This somehow doesn't get called if you don't map <C-C> to <C-[>.
function! s:callback(_id, _result) abort
  call s:hide_all()
  call clap#exit()
endfunction

function! s:remove_last_item(str) abort
  if len(a:str) < 2
    return ''
  endif
  let s = ''
  for idx in range(0, len(a:str)-2)
    let s .= a:str[idx]
  endfor
  return s
endfunction

function! s:mock_input() abort
  if s:input == ''
        \ || type(s:cursor_idx) ==# v:t_string
        \ || s:cursor_idx == strlen(s:input)
    let input = s:input.'|'
  elseif s:cursor_idx == 0
    let input = '|'.s:input
  else
    let input = join([s:input[:s:cursor_idx], s:input[s:cursor_idx+1:]], '|')
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
  call popup_move(s:preview_winid, #{col: col, line: line})

  call popup_show(s:preview_winid)
  call popup_settext(s:preview_winid, a:lines)
endfunction

function! s:apply_input(_timer) abort
  if g:clap.provider.is_async()
    call g:clap.provider.jobstop()
  endif
  call g:clap.provider.on_typed()
endfunction

function! s:apply_input_with_delay() abort
  if s:input_timer != -1
    call timer_stop(s:input_timer)
  endif
  let s:input_timer = timer_start(300, function('s:apply_input'))
endfunction

function! s:popup_filter(winid, key) abort
  if a:key == "\<C-J>"
    call win_execute(a:winid, 'call clap#handler#navigate_result("down")')

  elseif a:key == "\<C-K>"
    call win_execute(a:winid, 'call clap#handler#navigate_result("up")')

  elseif a:key == "\<Tab>"
    call win_execute(a:winid, 'call clap#handler#select_toggle()')

  " Ctrl-[ / Esc
  elseif a:key == "\<Esc>"
    call clap#handler#exit()

  elseif a:key == "\<CR>"
    call clap#handler#sink()

  elseif a:key == "\<C-A>"
    let s:cursor_idx = 0
    call s:mock_input()

  elseif a:key == "\<C-B>" || a:key == "\<Left>"
    let s:cursor_idx -= 1
    if s:cursor_idx < 0
      let s:cursor_idx = 0
    endif
    call s:mock_input()

  elseif a:key == "\<C-F>" || a:key == "\<Right>"
    let s:cursor_idx += 1
    let input_len = strlen(s:input)
    if s:cursor_idx > input_len
      let s:cursor_idx = input_len
    endif
    call s:mock_input()

  elseif a:key == "\<C-E>"
    let s:cursor_idx = strlen(s:input)
    call s:mock_input()

  elseif a:key == "\<BS>" || a:key == "\<C-D>"
    if empty(s:input) || s:cursor_idx == 0
      return 1
    endif
    let to_truncate = s:input[:s:cursor_idx]
    let truncated = s:remove_last_item(to_truncate)
    let s:input = truncated . s:input[s:cursor_idx+1:]
    let s:cursor_idx -= 1
    if s:cursor_idx < 0
      let s:cursor_idx = 0
    endif
    if g:clap.provider.is_sync()
      let g:__clap_should_refilter = v:true
    endif
    call g:clap.provider.on_typed()
    call s:mock_input()

  " ASCII printable characters
  elseif char2nr(a:key) >= 32 && char2nr(a:key) <= 126
    " FIXME still problematic
    if s:input == '' || s:cursor_idx == strlen(s:input)
      let s:input .= a:key
    else
      if s:cursor_idx == 0
        let s:input = a:key . s:input
      else
        let s:input = s:input[:s:cursor_idx].a:key.s:input[s:cursor_idx+1:]
      endif
    endif
    let s:cursor_idx += 1

    " If the privder is async, react immediately, otherwise hold a delay.
    " FIXME
    " If the slow renderring of vim job is resolved, this cuold be removed.
    if g:clap.provider.is_sync()
      " apply_input should happen earlier than mock_input
      call s:apply_input('')

      " FIXME s:mock_input would conflict with clap#indicator#set_matches()
      call s:mock_input()
    else
      call s:apply_input_with_delay()
      call s:mock_input()
    endif

  endif

  return 1
endfunction

function! s:open_popup() abort
  call s:create_display()

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
  call popup_settext(s:spinner_winid, clap#spinner#get())
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

  call g:clap.provider.init_display_win()

  let g:clap.display.initial_size = g:clap.display.line_count()

  if g:clap.provider.support_multi_selection()
    call win_execute(s:display_winid, 'setlocal signcolumn=yes')
  endif

  let g:clap_indicator_winid = s:indicator_winid

  call g:clap.provider.on_enter()

  " TODO more roboust?
  augroup ClapEnsureAllClosed
    autocmd!
    autocmd BufEnter,WinEnter,WinLeave * call clap#popup#close()
  augroup END

  silent doautocmd <nomodeline> User ClapOnEnter

  if has_key(g:clap.provider, 'args')
    call g:clap.input.set(join(g:clap.provider.args, ' '))
    call g:clap.provider.on_typed()
  endif
endfunction

function! clap#popup#close() abort
  call s:hide_all()
  silent autocmd! ClapEnsureAllClosed
endfunction
