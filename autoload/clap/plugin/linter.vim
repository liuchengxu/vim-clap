" Author: liuchengxu <xuliuchengxlc@gmail.com>

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim


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

function! s:render_diagnostics(bufnr, diagnostics) abort
  let extmark_ids = []

  for diagnostic in a:diagnostics
    try
      call nvim_buf_add_highlight(a:bufnr, s:linter_highlight_ns_id, 'ClapLinterUnderline', diagnostic.line_start - 1, diagnostic.column_start - 1, diagnostic.column_end - 1)

      if diagnostic.severity ==? 'error'
        let highlight = 'DiagnosticError'
        let message = '[E] '.diagnostic.message
      elseif diagnostic.severity ==? 'warning'
        let highlight = 'DiagnosticWarn'
        let message = '[W] '.diagnostic.message
      else
        let highlight = 'Normal'
        let message = diagnostic.message
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
  let extmark_ids = s:render_diagnostics(a:bufnr, a:diagnostics)

  let clap_linter = getbufvar(a:bufnr, 'clap_linter', {})
  call extend(clap_linter.extmark_ids, extmark_ids)

  call setbufvar(a:bufnr, 'clap_linter', clap_linter)
endfunction

function! clap#plugin#linter#refresh(bufnr, diagnostics) abort
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
