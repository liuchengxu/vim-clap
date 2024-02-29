" Author: liuchengxu <xuliuchengxlc@gmail.com>

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

function! clap#plugin#lsp#jump_to(location) abort
  execute 'edit!' a:location.path
  noautocmd call setpos('.', [bufnr(''), a:location.row, a:location.column, 0])
  normal! zz
endfunction

function! s:to_quickfix_entry(location) abort
  return { 'filename': a:location.path, 'lnum': a:location.row, 'col': a:location.column, 'text': a:location.text }
endfunction

function! clap#plugin#lsp#populate_quickfix(id, locations) abort
  let entries = map(a:locations, 's:to_quickfix_entry(v:val)')
  call clap#sink#open_quickfix(entries)
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

function! clap#plugin#lsp#reload(bufnr) abort
  call clap#client#notify('lsp.__reload', [a:bufnr])
endfunction

function! clap#plugin#lsp#detach(bufnr) abort
  call clap#client#notify('lsp.__detach', [a:bufnr])
endfunction

if has('nvim')
function! clap#plugin#lsp#on_lines(...) abort
  let [bufnr, changedtick, firstline, lastline, new_lastline] = a:000
  call clap#client#notify('lsp.__didChange', {
              \ 'bufnr': bufnr,
              \ 'changedtick': changedtick,
              \ 'firstline': firstline,
              \ 'lastline': lastline,
              \ 'new_lastline': new_lastline,
              \ })
endfunction

function! clap#plugin#lsp#buf_attach(bufnr) abort
    let g:__clap_buf_to_attach = a:bufnr
lua << END
  vim.api.nvim_buf_attach(vim.g.__clap_buf_to_attach, false, {
    on_lines = function(_lines, bufnr, changedtick, firstline, lastline, new_lastline)
      vim.api.nvim_call_function("clap#plugin#lsp#on_lines", { bufnr, changedtick, firstline, lastline, new_lastline })
    end,

    on_reload = function(_, bufnr)
      vim.api.nvim_call_function("clap#plugin#lsp#reload", {bufnr})
    end,

    on_detach = function(_, bufnr)
      vim.api.nvim_call_function("clap#plugin#lsp#detach", {bufnr})
    end
    })
END
endfunction

else

function! clap#plugin#lsp#listener(bufnr, start, end, added, changes) abort
  call clap#client#notify('lsp.__didChange', {
              \ 'bufnr': a:bufnr,
              \ 'start': a:start,
              \ 'end': a:end,
              \ 'added': a:added,
              \ 'changes': a:changes,
              \ 'changedtick': getbufvar(a:bufnr, 'changedtick'),
              \ })
endfunction

function! clap#plugin#lsp#buf_attach(bufnr) abort
  call listener_add('clap#plugin#lsp#listener')
endfunction

endif

let &cpoptions = s:save_cpo
unlet s:save_cpo
