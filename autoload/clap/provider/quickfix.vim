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

function! clap#provider#quickfix#into_qf_line(qf_entry) abort
  return s:qf_fmt_entry(a:qf_entry)
endfunction

function! clap#provider#quickfix#extract_position(selected) abort
  return s:extract_position(a:selected)
endfunction

function! s:qf_fmt_entry(qf_entry) abort
  let path = bufname(a:qf_entry['bufnr'])
  let line_col = a:qf_entry['lnum'].' col '.a:qf_entry['col']
  return path.'|'.line_col.'| '.trim(s:qf_fmt_text(a:qf_entry['text']))
endfunction

function! s:qf_fmt_text(text) abort
  return substitute(a:text, '\n\( \|\t\)*', ' ', 'g')
endfunction

function! s:extract_position(selected) abort
  let [fpath, line_col] = split(a:selected, '|')[:1]
  let [lnum, column] = split(line_col, ' col ')
  return [fpath, lnum, column]
endfunction

function! s:quickfix.sink(selected) abort
  let [fpath, lnum, column] = s:extract_position(a:selected)
  execute 'edit' fpath
  noautocmd call cursor(lnum, column)
endfunction

let s:quickfix.syntax = 'qf'
let g:clap#provider#quickfix# = s:quickfix

let &cpoptions = s:save_cpo
unlet s:save_cpo
