" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Make a compatible layer between neovim and vim.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:is_nvim = has('nvim')

" Returns the original full line with icon if it was added by maple given
" the lnum of display buffer.
function! clap#api#get_origin_line_at(lnum) abort
  if exists('g:__clap_lines_truncated_map')
        \ && has_key(g:__clap_lines_truncated_map, a:lnum)
    return g:__clap_lines_truncated_map[a:lnum]
  else
    return get(getbufline(g:clap.display.bufnr, a:lnum), 0, '')
  endif
endfunction

if exists('*win_execute')
  function! clap#api#win_execute(winid, command) abort
    return win_execute(a:winid, a:command)
  endfunction
else
  function! clap#api#win_execute(winid, command) abort
    let cur_winid = bufwinid('')
    if cur_winid != a:winid
      noautocmd call win_gotoid(a:winid)
      try
        return execute(a:command)
      finally
        noautocmd call win_gotoid(cur_winid)
      endtry
    else
      return execute(a:command)
    endif
  endfunction
endif

function! clap#api#update_winbar(winid, winbar, winbar_hl) abort
  if winheight(a:winid) < 2
    return 0
  endif
  if empty(a:winbar)
    let l:winbar = ''
  else
    let l:winbar = escape(a:winbar, ' ')
  endif
  call clap#api#win_execute(a:winid, 'setlocal winbar='.l:winbar)
  if !exists('s:winbar_hl_initialized')
    call execute('hi! link WinBar '.a:winbar_hl)
    call execute('hi! link WinBarNC '.a:winbar_hl)
    let s:winbar_hl_initialized = v:true
  endif
endfunction

function! clap#api#on_click_function_tag(minwid, clicks, button, mods) abort
  call clap#client#notify('ctags.__onClickFunctionTag', {})
endfunction

" Show the message in a popup at the right-top corner.
function! clap#api#popup_notify(messages, timeout) abort
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

let s:api = {}

if s:is_nvim
  function! clap#api#buf_set_lines(bufnr, lines) abort
    call nvim_buf_set_lines(a:bufnr, 0, -1, 0, a:lines)
  endfunction

  function! clap#api#buf_clear(bufnr) abort
    call nvim_buf_set_lines(a:bufnr, 0, -1, 0, [])
  endfunction

  function! s:api.win_is_valid(winid) abort
    return nvim_win_is_valid(a:winid)
  endfunction

  function! s:api.buf_is_valid(buf) abort
    return nvim_buf_is_valid(a:buf)
  endfunction

  function! s:api.get_var(name) abort
    return nvim_get_var(a:name)
  endfunction
else
  function! clap#api#buf_set_lines(bufnr, lines) abort
    " silent is required to avoid the annoying --No lines in buffer--.
    silent call deletebufline(a:bufnr, 1, '$')

    call appendbufline(a:bufnr, 0, a:lines)
    " Delete the last possible empty line.
    " Is there a better solution in vim?
    if empty(getbufline(a:bufnr, '$')[0])
      silent call deletebufline(a:bufnr, '$')
    endif
  endfunction

  function! clap#api#buf_clear(bufnr) abort
    silent call deletebufline(a:bufnr, 1, '$')
  endfunction

  function! s:api.win_is_valid(winid) abort
    return win_screenpos(a:winid) != [0, 0]
  endfunction

  function! s:api.buf_is_valid(buf) abort
    return bufexists(a:buf) ? v:true : v:false
  endfunction

  function! s:api.get_var(name) abort
    return get(g:, a:name, v:null)
  endfunction
endif

function! s:api.get_screen_lines_range() abort
  return [win_getid(), line('w0'), line('w$')]
endfunction

function! s:api.get_cursor_pos() abort
  let [_, row, column, _] = getpos('.')
  return [bufnr(), row, column]
endfunction

" The leading icon is stripped.
function! s:api.display_getcurline() abort
  return [g:clap.display.getcurline(), get(g:, '__clap_icon_added_by_maple', v:false)]
endfunction

function! s:api.display_set_lines(lines) abort
  call g:clap.display.set_lines(a:lines)
endfunction

function! s:api.provider_source() abort
  if has_key(g:clap.provider, 'source_type') && has_key(g:clap.provider._(), 'source')
    if g:clap.provider.source_type == g:__t_string
      return [g:clap.provider._().source]
    elseif g:clap.provider.source_type == g:__t_func_string
      return [g:clap.provider._().source()]
    elseif g:clap.provider.source_type == g:__t_list
      return [g:clap.provider._().source]
    elseif g:clap.provider.source_type == g:__t_func_list
      " Note that this function call should always be pretty fast and not slow down Vim.
      return [g:clap.provider._().source()]
    endif
  endif
  return []
endfunction

function! s:api.provider_source_cmd() abort
  if has_key(g:clap.provider, 'source_type') && has_key(g:clap.provider._(), 'source')
    if g:clap.provider.source_type == g:__t_string
      return [g:clap.provider._().source]
    elseif g:clap.provider.source_type == g:__t_func_string
      return [g:clap.provider._().source()]
    endif
  endif
  return []
endfunction

function! s:api.provider_args() abort
  return get(g:clap.provider, 'args', [])
endfunction

function! s:api.input_set(value) abort
  call g:clap.input.set(a:value)
endfunction

function! s:api.set_var(name, value) abort
  execute 'let '.a:name.'= a:value'
endfunction

function! s:api.current_buffer_path() abort
  return expand('#'.bufnr('%').':p')
endfunction

function! s:api.matchdelete_batch(match_ids, winid) abort
  silent! call map(a:match_ids, 'matchdelete(v:val, a:winid)')
endfunction

function! s:api.curbufline(lnum) abort
  return get(getbufline(bufnr(''), a:lnum), 0, v:null)
endfunction

function! s:api.append_and_write(lnum, text) abort
  call append(a:lnum, a:text)
  silent noautocmd write
endfunction

function! s:api.show_lines_in_preview(lines) abort
  if type(a:lines) is v:t_string
    call g:clap.preview.show([a:lines])
  else
    call g:clap.preview.show(a:lines)
  endif
endfunction

function! s:api.echomsg(msg) abort
  echomsg a:msg
endfunction

function! s:api.verbose(cmd) abort
  redir => l:output
  silent execute ':verbose' a:cmd
  redir END
  return l:output
endfunction

function! s:api.set_initial_query(query) abort
  if a:query ==# '@visual'
    let query = clap#util#get_visual_selection()
  else
    let query = clap#util#expand(a:query)
  endif

  if s:is_nvim
    call feedkeys(query)
  else
    call g:clap.input.set(query)
    " Move the cursor to the end.
    call feedkeys("\<C-E>", 'xt')
  endif

  return query
endfunction

function! clap#api#call(method, args) abort
  " Catch all the exceptions
  try
    if has_key(s:api, a:method)
      return call(s:api[a:method], a:args)
    else
      return call(a:method, a:args)
    endif
  catch /^Vim:Interrupt$/ " catch interrupts (CTRL-C)
  catch
    echoerr printf('[clap#api#call] method: %s, args: %s, exception: %s', a:method, string(a:args), v:exception)
  endtry
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
