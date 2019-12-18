" Author: KITAGAWA Yasutaka <kit494way@gmail.com>
" Description: List the entries of the quickfix list.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:quickfix = {}

function! s:quickfix.source() abort
  " Ignore quickfix list entries with non-existing buffer number.
  let qflist = filter(getqflist(), 'v:val["bufnr"]')

  return map(qflist, 's:to_grepformat(v:val)')
endfunction

function! s:to_grepformat(quickfix) abort
  let path = bufname(a:quickfix['bufnr'])
  let line_col = a:quickfix['lnum'].' col '.a:quickfix['col']
  return path.'|'.line_col.'|'.a:quickfix['text']
endfunction

function! s:quickfix.sink(selected) abort
  let [fpath, line_col] = split(a:selected, '|')[:1]
  let [lnum, column] = split(line_col, ' col ')

  execute 'edit' fpath
  noautocmd call cursor(lnum, column)
endfunction

let s:quickfix.enable_rooter = v:true

let g:clap#provider#quickfix# = s:quickfix

let &cpoptions = s:save_cpo
unlet s:save_cpo
