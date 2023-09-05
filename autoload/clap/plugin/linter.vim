" Author: liuchengxu <xuliuchengxlc@gmail.com>

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:preview_winhl = 'Normal:ErrorMsg,EndOfBuffer:ClapPreviewInvisibleEndOfBuffer,SignColumn:ClapPreview,ColorColumn:ClapPreview'

hi ClapLinterUnderline cterm=underline,bold gui=undercurl,italic,bold ctermfg=173 guifg=#e18254

hi default link DiagnosticError ErrorMsg
hi DiagnosticWarn ctermfg=136 guifg=#b1951d
hi default link DiagnosticInfo Normal
hi default link DiagnosticHint Normal

if !exists('s:linter_ns_id')
  let s:linter_ns_id = nvim_create_namespace('clap_linter')
endif

if !exists('s:linter_highlight_ns_id')
  let s:linter_highlight_ns_id = nvim_create_namespace('clap_linter_highlight')
endif

function! s:display_on_top_right(lines) abort
  if !exists('s:diagnostic_info_buffer') || !nvim_buf_is_valid(s:diagnostic_info_buffer)
    let s:diagnostic_info_buffer = nvim_create_buf(v:false, v:true)
  endif

  if exists('s:diagnostic_info_winid') &&  nvim_win_is_valid(s:diagnostic_info_winid)
    " Win is already open.
  else
    let config = {
          \ 'relative': 'win',
          \ 'win': nvim_get_current_win(),
          \ 'row': 0,
          \ 'col': winwidth(0),
          \ 'width': max(map(a:lines, 'strlen(v:val)')),
          \ 'height': 1,
          \ 'style': 'minimal',
          \ 'border': 'single',
          \ 'anchor': 'NE',
          \ 'focusable': v:false,
          \ }

    silent let s:diagnostic_info_winid = nvim_open_win(s:diagnostic_info_buffer, v:false, config)

    call setwinvar(s:diagnostic_info_winid, '&spell', 0)
    call setwinvar(s:diagnostic_info_winid, '&winhl', s:preview_winhl)

    call setbufvar(s:diagnostic_info_buffer, '&signcolumn', 'no')
  endif

  call nvim_buf_set_lines(s:diagnostic_info_buffer, 0, -1, v:false, a:lines)
endfunction

function! clap#plugin#linter#clear_top_right() abort
  if exists('s:diagnostic_info_winid')
    call nvim_win_close(s:diagnostic_info_winid, v:true)
    unlet s:diagnostic_info_winid
  endif
endfunction

function! clap#plugin#linter#display_top_right(current_diagnostics) abort
  if !empty(a:current_diagnostics)
    let messages = []
    for diagnostic in a:current_diagnostics
      let code = empty(diagnostic.code) ? '' : ' ['.diagnostic.code.']'
      let message = printf('%s: %s%s', diagnostic.severity, diagnostic.message, code)
      call add(messages, message)
    endfor

    call s:display_on_top_right(messages)
  endif
endfunction

function! s:render_diagnostics(bufnr, diagnostics) abort
  let extmark_ids = []

  for diagnostic in a:diagnostics
    try
      call nvim_buf_add_highlight(a:bufnr, s:linter_highlight_ns_id, 'ClapLinterUnderline', diagnostic.line_start - 1, diagnostic.column_start - 1, diagnostic.column_end - 1)

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
