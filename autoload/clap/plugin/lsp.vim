" Author: liuchengxu <xuliuchengxlc@gmail.com>

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

function! clap#plugin#lsp#jump_to(location) abort
  execute 'edit' a:location.path
  noautocmd call setpos('.', [bufnr(''), a:location.row, a:location.column, 0])
endfunction

function! clap#plugin#lsp#open_picker(id, locations) abort
  let provider = {
        \ 'id': a:id,
        \ 'source': map(a:locations, 'v:val["path"]:v:val["row"]:v:val["column"]'),
        \ 'sink': 'e',
        \ }
  call clap#run(provider)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
