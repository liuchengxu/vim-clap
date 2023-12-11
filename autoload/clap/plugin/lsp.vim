" Author: liuchengxu <xuliuchengxlc@gmail.com>

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

function! clap#plugin#lsp#jump_to(path, row, column) abort
  execute 'edit' a:path
  noautocmd call setpos('.', [bufnr(''), a:row, a:column, 0])
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
