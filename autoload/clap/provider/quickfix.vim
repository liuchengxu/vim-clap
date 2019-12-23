" Author: KITAGAWA Yasutaka <kit494way@gmail.com>
" Description: List the entries of the quickfix list.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:quickfix = {}

function! s:quickfix.source() abort
  " Ignore quickfix list entries with non-existing buffer number.
  let qflist = filter(getqflist(), 'v:val["bufnr"]')

  return map(qflist, 's:qf_fmt_entry(v:val)')
endfunction

function! s:qf_fmt_entry(qf_entry) abort
  let path = bufname(a:qf_entry['bufnr'])
  let line_col = a:qf_entry['lnum'].' col '.a:qf_entry['col']
  return path.'|'.line_col.'| '.trim(s:qf_fmt_text(a:qf_entry['text']))
endfunction

function! s:qf_fmt_text(text) abort
  return substitute(a:text, '\n\( \|\t\)*', ' ', 'g')
endfunction

function! s:quickfix.sink(selected) abort
  let [fpath, line_col] = split(a:selected, '|')[:1]
  let [lnum, column] = split(line_col, ' col ')

  execute 'edit' fpath
  noautocmd call cursor(lnum, column)
endfunction

function! s:quickfix.on_enter() abort
  call g:clap.display.setbufvar('&syntax', 'qf')
endfunction

let g:clap#provider#quickfix# = s:quickfix

let &cpoptions = s:save_cpo
unlet s:save_cpo
