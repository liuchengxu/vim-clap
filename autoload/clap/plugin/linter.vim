" Author: liuchengxu <xuliuchengxlc@gmail.com>

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

hi ClapLinterUnderline cterm=underline,bold gui=undercurl,italic,bold ctermfg=173 guifg=#e18254

hi DiagnosticWarn ctermfg=136 guifg=#b1951d

hi default link DiagnosticError ErrorMsg
hi default link DiagnosticInfo Normal
hi default link DiagnosticHint Normal

let s:diagnostic_winhl = 'Normal:Pmenu,EndOfBuffer:ClapPreviewInvisibleEndOfBuffer'

if !exists('s:linter_ns_id')
  let s:linter_ns_id = nvim_create_namespace('clap_linter')
endif

if !exists('s:linter_highlight_ns_id')
  let s:linter_highlight_ns_id = nvim_create_namespace('clap_linter_highlight')
endif

if !exists('s:linter_msg_highlight_ns_id')
  let s:linter_msg_highlight_ns_id = nvim_create_namespace('clap_linter_msg_highlight')
endif

function! s:display_on_top_right(lines, line_highlights) abort
  if !exists('s:diagnostic_msg_buffer') || !nvim_buf_is_valid(s:diagnostic_msg_buffer)
    let s:diagnostic_msg_buffer = nvim_create_buf(v:false, v:true)
  endif
  let buffer = s:diagnostic_msg_buffer

  if !exists('s:diagnostic_msg_winid') || !nvim_win_is_valid(s:diagnostic_msg_winid)
    " Clear the invalid ones.
    call clap#plugin#linter#clear_top_right()

    let max_text_len = max(map(copy(a:lines), 'strlen(v:val)'))

    if max_text_len > &columns
      let width = &columns / 2
      let height = len(a:lines) + max_text_len / width
    else
      let width = max_text_len
      let height = len(a:lines)
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

    silent let s:diagnostic_msg_winid = nvim_open_win(buffer, v:false, config)

    call setwinvar(s:diagnostic_msg_winid, '&spell', 0)
    call setwinvar(s:diagnostic_msg_winid, '&wrap', 1)
    call setwinvar(s:diagnostic_msg_winid, '&winhl', s:diagnostic_winhl)
  endif

  call nvim_buf_set_lines(buffer, 0, -1, v:false, a:lines)

  let line = 0
  for line_highlight in a:line_highlights
    for [highlight_group, col_start, col_end] in line_highlight
      call nvim_buf_add_highlight(buffer, s:linter_msg_highlight_ns_id, highlight_group, line, col_start, col_end)
    endfor
    let line += 1
  endfor
endfunction

function! clap#plugin#linter#clear_top_right() abort
  if exists('s:diagnostic_msg_winid')
    call nvim_win_close(s:diagnostic_msg_winid, v:true)
    call nvim_buf_clear_namespace(s:diagnostic_msg_buffer, s:linter_msg_highlight_ns_id, 0, -1)
    unlet s:diagnostic_msg_winid
  endif
endfunction

function! clap#plugin#linter#display_top_right(current_diagnostics) abort
  if !empty(a:current_diagnostics)
    let messages = []
    let message_highlights = []
    for diagnostic in a:current_diagnostics
      let code = empty(diagnostic.code) ? '' : ' ['.diagnostic.code.']'

      let highlights = []

      let severity_len = strlen(diagnostic.severity)
      call add(highlights, ['Error', 0, severity_len])

      let message_len = strlen(diagnostic.message)
      call add(highlights, ['Comment', severity_len + 2, severity_len + 2 + message_len])

      let code_len = strlen(code)
      call add(highlights, ['Title', severity_len + 2 + message_len, severity_len + 2 + message_len + code_len])
      call add(message_highlights, highlights)

      let message = printf('%s: %s%s ', diagnostic.severity, diagnostic.message, code)
      call add(messages, message)
    endfor

    call s:display_on_top_right(messages, message_highlights)
  endif
endfunction

function! s:render_diagnostics(bufnr, diagnostics) abort
  let extmark_ids = []

  let skip_eol_highlight = v:true

  for diagnostic in a:diagnostics
    try
      call nvim_buf_add_highlight(a:bufnr, s:linter_highlight_ns_id, 'ClapLinterUnderline', diagnostic.line_start - 1, diagnostic.column_start - 1, diagnostic.column_end - 1)

      if skip_eol_highlight
        continue
      endif

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
      let id = nvim_buf_set_extmark(a:bufnr, s:linter_ns_id, diagnostic.line_start - 1, diagnostic.column_end - 1, opts)
      call add(extmark_ids, id)

    " Suppress error: Invalid 'col': out of range
    catch /^Vim\%((\a\+)\)\=:E5555/
      echom v:exception.', diagnostic:'.string(diagnostic)
    endtry
  endfor

  return extmark_ids
endfunction

function! clap#plugin#linter#update(bufnr, diagnostics) abort
  call extend(g:clap_linter, a:diagnostics)
  let extmark_ids = s:render_diagnostics(a:bufnr, a:diagnostics)

  let clap_linter = getbufvar(a:bufnr, 'clap_linter', {})
  if has_key(clap_linter, 'extmark_ids')
    call extend(clap_linter.extmark_ids, extmark_ids)
    call setbufvar(a:bufnr, 'clap_linter', clap_linter)
  endif
endfunction

function! clap#plugin#linter#refresh(bufnr, diagnostics) abort
  let g:clap_linter = a:diagnostics
  call clap#plugin#linter#clear(a:bufnr)

  let extmark_ids = s:render_diagnostics(a:bufnr, a:diagnostics)

  call setbufvar(a:bufnr, 'clap_linter', { 'extmark_ids': extmark_ids })
endfunction

function! clap#plugin#linter#clear(bufnr) abort
  let clap_linter = getbufvar(a:bufnr, 'clap_linter', {})
  for id in get(clap_linter, 'extmark_ids', [])
    call nvim_buf_del_extmark(a:bufnr, s:linter_ns_id, id)
  endfor
  call nvim_buf_clear_namespace(a:bufnr, s:linter_highlight_ns_id, 0, -1)

  call setbufvar(a:bufnr, 'clap_linter', {})
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
