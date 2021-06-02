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

function! s:quickfix.on_move() abort
  let [fpath, lnum, column] = s:extract_position(g:clap.display.getcurline())

  if lnum == 0 || column == 0
    call clap#preview#file(fpath)
  else
    call clap#preview#file_at(fpath, lnum)
  endif
endfunction

function! s:quickfix.on_move_async() abort
  call clap#client#call('quickfix', function('clap#impl#on_move#handler'), {
        \ 'curline': g:clap.display.getcurline(),
        \ 'cwd': clap#rooter#working_dir(),
        \ 'winwidth': winwidth(g:clap.display.winid),
        \ 'winheight': winheight(g:clap.display.winid),
        \ })
endfunction

let s:quickfix.syntax = 'qf'
let g:clap#provider#quickfix# = s:quickfix

let &cpoptions = s:save_cpo
unlet s:save_cpo
