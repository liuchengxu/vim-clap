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

function! clap#plugin#lsp#open_picker() abort
    let provider = {
          \ 'id': 'lsp',
          \ 'on_typed': { -> clap#client#notify_provider('on_typed') },
          \ 'sink': 'e',
          \ }
    call clap#run(provider)
endfunction

function! clap#plugin#lsp#provider_context() abort
  let params = {
        \   'provider_id': 'lsp',
        \   'input': { 'bufnr': g:clap.input.bufnr, 'winid': g:clap.input.winid },
        \   'start': { 'bufnr': g:clap.start.bufnr, 'winid': g:clap.start.winid },
        \   'display': { 'bufnr': g:clap.display.bufnr, 'winid': g:clap.display.winid },
        \   'cwd': clap#rooter#working_dir(),
        \   'icon': g:clap_enable_icon ? get(g:clap.provider._(), 'icon', 'Null') : 'Null',
        \   'no_cache': has_key(g:clap.context, 'no-cache') ? v:true : v:false,
        \   'start_buffer_path': expand('#'.g:clap.start.bufnr.':p'),
        \   'source_is_list': v:false,
        \ }
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
