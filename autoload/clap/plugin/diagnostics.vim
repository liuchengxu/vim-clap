" Author: liuchengxu <xuliuchengxlc@gmail.com>

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:severity_icons = {
      \ 'error': '  ',
      \ 'warning': '  ',
      \ 'hint': '  ',
      \ 'info': '  ',
      \ 'other': '  ',
      \ }

hi ClapDiagnosticUnderlineWarn  cterm=underline,bold gui=undercurl,italic,bold ctermfg=173 guifg=#e18254
hi ClapDiagnosticUnderlineError cterm=underline,bold gui=undercurl,italic,bold ctermfg=196 guifg=#f2241f

hi DiagnosticWarn ctermfg=136 guifg=#b1951d

hi default link DiagnosticError ErrorMsg
hi default link DiagnosticInfo Normal
hi default link DiagnosticHint Normal

function! s:convert_diagnostics_to_lines(current_diagnostics) abort
  let lines = []
  let line_highlights = []
  for diagnostic in a:current_diagnostics
    let code = empty(diagnostic.code) ? '' : ' ['.diagnostic.code.']'

    let highlights = []

    let severity_icon = get(s:severity_icons, diagnostic.severity, s:severity_icons.other)
    let severity_len = strlen(severity_icon)

    call add(highlights, ['DiagnosticError', 0, severity_len])

    let message_len = strlen(diagnostic.message)
    " 1 = ` `
    let offset = severity_len + 1
    call add(highlights, ['Comment', offset, offset + message_len])

    let code_len = strlen(code)
    let offset = offset + message_len
    call add(highlights, ['Title', offset, offset + code_len])
    call add(line_highlights, highlights)

    let line = printf('%s %s%s', severity_icon, diagnostic.message, code)
    call add(lines, line)
  endfor
  return [lines, line_highlights]
endfunction

function! s:max_available_length_on_top() abort
  let win_line_start = line('w0')
  let max_top_line_len = max(map([win_line_start, win_line_start+1, win_line_start+2], "strlen(get(getbufline('', v:val), 0, ''))"))
  let line_cap = &columns - &numberwidth
  if &signcolumn ==# 'yes'
    let line_cap -= 2
  endif

  " Buy some more space for the code display.
  let line_cap -= 4

  return line_cap - max_top_line_len
endfunction

if has('nvim')

let s:diagnostic_winhl = 'Normal:Pmenu'
let s:diagnostic_eol_ns_id = nvim_create_namespace('clap_diagnostics_eol')
let s:diagnostic_spans_highlight_ns_id = nvim_create_namespace('clap_diagnostics_spans_highlight')
let s:diagnostic_msg_highlight_ns_id = nvim_create_namespace('clap_diagnostics_msg_highlight')

function! s:render_on_top_right(lines, line_highlights) abort
  if !exists('s:diagnostic_msg_buffer') || !nvim_buf_is_valid(s:diagnostic_msg_buffer)
    let s:diagnostic_msg_buffer = nvim_create_buf(v:false, v:true)
  endif
  let buffer = s:diagnostic_msg_buffer

  let max_text_len = max(map(copy(a:lines), 'strlen(v:val)'))

  let container_width = winwidth(0)

  if max_text_len > container_width
    let width = container_width / 2
    let height = len(a:lines) + max_text_len / width
  else
    let width = max_text_len
    let height = len(a:lines)
  endif

  " Make sure the diagnostic win won't interfere with the existing code
  " display.
  let max_available_on_top = s:max_available_length_on_top()
  if max_available_on_top > 0 && width > max_available_on_top
    let width = max_available_on_top
    let height += 1
  endif

  let config = {
        \ 'relative': 'win',
        \ 'win': nvim_get_current_win(),
        \ 'row': 0,
        \ 'col': winwidth(0),
        \ 'width': width,
        \ 'height': height,
        \ 'style': 'minimal',
        \ 'border': 'single',
        \ 'anchor': 'NE',
        \ 'focusable': v:false,
        \ }

  if !exists('s:diagnostic_msg_winid')
    silent let s:diagnostic_msg_winid = nvim_open_win(buffer, v:false, config)
    call setwinvar(s:diagnostic_msg_winid, '&spell', 0)
    call setwinvar(s:diagnostic_msg_winid, '&wrap', 1)
    call setwinvar(s:diagnostic_msg_winid, '&winhl', s:diagnostic_winhl)
  else
    if nvim_win_is_valid(s:diagnostic_msg_winid)
      call nvim_win_set_config(s:diagnostic_msg_winid, config)
    else
      " Make sure the invalid window is closed and create a new one.
      call clap#plugin#diagnostics#close_top_right()

      silent let s:diagnostic_msg_winid = nvim_open_win(buffer, v:false, config)
      call setwinvar(s:diagnostic_msg_winid, '&spell', 0)
      call setwinvar(s:diagnostic_msg_winid, '&wrap', 1)
      call setwinvar(s:diagnostic_msg_winid, '&winhl', s:diagnostic_winhl)
    endif
  endif

  call nvim_buf_set_lines(buffer, 0, -1, v:false, a:lines)

  let line = 0
  for line_highlight in a:line_highlights
    for [highlight_group, col_start, col_end] in line_highlight
      call nvim_buf_add_highlight(buffer, s:diagnostic_msg_highlight_ns_id, highlight_group, line, col_start, col_end)
    endfor
    let line += 1
  endfor
endfunction

function! clap#plugin#diagnostics#close_top_right() abort
  if exists('s:diagnostic_msg_winid')
    call nvim_win_close(s:diagnostic_msg_winid, v:true)
    call nvim_buf_clear_namespace(s:diagnostic_msg_buffer, s:diagnostic_msg_highlight_ns_id, 0, -1)
    unlet s:diagnostic_msg_winid
  endif
endfunction

function! clap#plugin#diagnostics#display_top_right(current_diagnostics) abort
  if !empty(a:current_diagnostics)
    let [lines, line_highlights] = s:convert_diagnostics_to_lines(a:current_diagnostics)
    call s:render_on_top_right(lines, line_highlights)
  endif
endfunction

function! s:highlight_span(bufnr, span, hl_group) abort
  try
    call nvim_buf_add_highlight(a:bufnr, s:diagnostic_spans_highlight_ns_id, a:hl_group, a:span.line_start - 1, a:span.column_start - 1, a:span.column_end - 1)
  catch
    " Span may be invalid at this moment since the buffer has been changed.
    return
  endtry
endfunction

function! s:render_eol(bufnr, diagnostics) abort
  let extmark_ids = []

  for diagnostic in a:diagnostics
    try
      let code = empty(diagnostic.code) ? '' : ' '.diagnostic.code

      if diagnostic.severity ==? 'error'
        let highlight = 'DiagnosticError'
        let message = printf('[E] %s%s', diagnostic.message, code)
      elseif diagnostic.severity ==? 'warning'
        let highlight = 'DiagnosticWarn'
        let message = printf('[W] %s%s', diagnostic.message, code)
      else
        let highlight = 'Normal'
        let message = printf('%s%s', diagnostic.message, code)
      endif

      let opts = { 'virt_text': [[message, highlight]], 'virt_text_pos': 'eol' }
      let id = nvim_buf_set_extmark(a:bufnr, s:diagnostic_eol_ns_id, diagnostic.line_start - 1, diagnostic.column_end - 1, opts)
      call add(extmark_ids, id)

    " Suppress error: Invalid 'col': out of range
    catch /^Vim\%((\a\+)\)\=:E5555/
      echom v:exception.', diagnostic:'.string(diagnostic)
    endtry
  endfor

  return extmark_ids
endfunction

function! s:add_eol(bufnr, diagnostics) abort
  let extmark_ids = s:render_eol(a:bufnr, a:diagnostics)

  let clap_diagnostics = getbufvar(a:bufnr, 'clap_diagnostics', {})
  if has_key(clap_diagnostics, 'extmark_ids')
    call extend(clap_diagnostics.extmark_ids, extmark_ids)
    call setbufvar(a:bufnr, 'clap_diagnostics', clap_diagnostics)
  endif
endfunction

function! s:refresh_eol(bufnr, diagnostics) abort
  let extmark_ids = s:render_eol(a:bufnr, a:diagnostics)
  call setbufvar(a:bufnr, 'clap_diagnostics', { 'extmark_ids': extmark_ids })
endfunction

function! s:delete_eol(bufnr) abort
  let clap_diagnostics = getbufvar(a:bufnr, 'clap_diagnostics', {})
  for id in get(clap_diagnostics, 'extmark_ids', [])
    call nvim_buf_del_extmark(a:bufnr, s:diagnostic_eol_ns_id, id)
  endfor
  call setbufvar(a:bufnr, 'clap_diagnostics', {})
endfunction

function! clap#plugin#diagnostics#delete_highlights(bufnr) abort
  call nvim_buf_clear_namespace(a:bufnr, s:diagnostic_spans_highlight_ns_id, 0, -1)
endfunction

else

function! s:highlight_span(bufnr, span, hl_group) abort
  let length = a:span.column_end - a:span.column_start
  " It's possible that the span across multiple lines and this can be negative.
  if length < 0
    let length = 1
  endif
  try
    call prop_add(a:span.line_start, a:span.column_start, { 'type': a:hl_group, 'length': length, 'bufnr': a:bufnr })
  catch
    " Span may be invalid at this moment since the buffer has been changed.
    return
  endtry
endfunction

function! clap#plugin#diagnostics#close_top_right() abort
  if exists('s:diagnostic_msg_winid')
    call popup_close(s:diagnostic_msg_winid)
    unlet s:diagnostic_msg_winid
  endif
endfunction

function! clap#plugin#diagnostics#display_top_right(current_diagnostics) abort
  if !empty(a:current_diagnostics)
    let [lines, line_highlights] = s:convert_diagnostics_to_lines(a:current_diagnostics)

    let max_text_len = max(map(copy(lines), 'strlen(v:val)'))
    if &columns > max_text_len
      let col = &columns - max_text_len
    else
      let col = 1
    endif

    let height = len(lines)

    " Make sure the diagnostic win won't interfere with the existing code
    " display.
    let max_available_on_top = s:max_available_length_on_top()
    if max_text_len > max_available_on_top
      let maxwidth = max_available_on_top
      let col += max_text_len - max_available_on_top
    else
      let maxwidth = max_text_len
    endif

    if exists('s:diagnostic_msg_winid') && !empty(popup_getpos(s:diagnostic_msg_winid))
      call popup_setoptions(s:diagnostic_msg_winid, { 'minheight': len(lines), 'col': col })
      call popup_settext(s:diagnostic_msg_winid, lines)
    else
      call clap#plugin#diagnostics#close_top_right()

      silent let s:diagnostic_msg_winid = popup_create(lines, {
            \ 'zindex': 100,
            \ 'title': 'Diagnostics',
            \ 'mapping': v:false,
            \ 'line': 1,
            \ 'col': col,
            \ 'pos': 'topleft',
            \ 'scrollbar': 0,
            \ 'maxwidth': maxwidth,
            \ 'minheight': height,
            \ 'border': [],
            \ 'borderchars': ['─', '│', '─', '│', '┌', '┐', '┘', '└'],
            \ })
    endif
  endif
endfunction

call prop_type_add('ClapDiagnosticUnderlineWarn', {'highlight': 'ClapDiagnosticUnderlineWarn'})
call prop_type_add('ClapDiagnosticUnderlineError', {'highlight': 'ClapDiagnosticUnderlineError'})

function! clap#plugin#diagnostics#delete_highlights(bufnr) abort
  call prop_remove({ 'type': 'ClapDiagnosticUnderlineWarn', 'bufnr': a:bufnr } )
  call prop_remove({ 'type': 'ClapDiagnosticUnderlineError', 'bufnr': a:bufnr } )
endfunction

endif

function! clap#plugin#diagnostics#add_highlights(bufnr, diagnostics) abort
  for diagnostic in a:diagnostics
    let hl_group = diagnostic.severity ==# 'Error' ? 'ClapDiagnosticUnderlineError' : 'ClapDiagnosticUnderlineWarn'
    call map(diagnostic.spans, 's:highlight_span(a:bufnr, v:val, hl_group)')
  endfor
endfunction

function! clap#plugin#diagnostics#refresh_highlights(bufnr, diagnostics) abort
  call clap#plugin#diagnostics#delete_highlights(a:bufnr)
  call clap#plugin#diagnostics#add_highlights(a:bufnr, a:diagnostics)
endfunction

function! clap#plugin#diagnostics#toggle_off(bufnr) abort
  call clap#plugin#diagnostics#delete_highlights(a:bufnr)
  call clap#plugin#diagnostics#close_top_right()
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
