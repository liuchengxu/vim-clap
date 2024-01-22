" Author: liuchengxu <xuliuchengxlc@gmail.com>

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

function! s:jump_to(location) abort
  execute 'edit' a:location.path
  noautocmd call setpos('.', [bufnr(''), a:location.row, a:location.column, 0])
  normal! zz
endfunction

function! s:to_quickfix_entry(location) abort
  return { 'filename': a:location.path, 'lnum': a:location.row, 'col': a:location.column, 'text': a:location.text }
endfunction

function! clap#plugin#lsp#handle_locations(id, locations) abort
  if len(a:locations) == 1
    call s:jump_to(a:locations[0])
    return
  endif

  let mode = 'quickfix'

  if mode ==# 'quickfix'
    let entries = map(a:locations, 's:to_quickfix_entry(v:val)')
    call clap#sink#open_quickfix(entries)
  else
    let provider = {
          \ 'id': a:id,
          \ 'source': map(a:locations, 'printf("%s:%s:%s", v:val["path"], v:val["row"], v:val["column"])'),
          \ 'sink': 'e',
          \ }

    call clap#run(provider)
  endif
endfunction

function! clap#plugin#lsp#open_picker(title) abort
  let provider = {
        \ 'id': 'lsp',
        \ 'title': a:title,
        \ 'on_typed': { -> clap#client#notify_provider('on_typed') },
        \ 'on_move': { -> clap#client#notify_provider('on_move') },
        \ 'sink': 'e',
        \ 'icon': 'lsp',
        \ }
  call clap#run(provider)
endfunction

function! clap#plugin#lsp#tab_size(bufnr) abort
    let l:shiftwidth = getbufvar(a:bufnr, '&shiftwidth')
    if getbufvar(a:bufnr, '&shiftwidth')
        return l:shiftwidth
    endif
    return getbufvar(a:bufnr, '&tabstop')
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
