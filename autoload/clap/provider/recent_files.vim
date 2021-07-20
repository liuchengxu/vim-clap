" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Persistent recent files.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:recent_files = {}

function! s:recent_files.on_typed() abort
  call clap#client#call('recent_files/on_typed', function('clap#state#handle_response_on_typed'), {
        \ 'provider_id': g:clap.provider.id,
        \ 'query': g:clap.input.get(),
        \ })
endfunction

function! s:recent_files.on_move_async() abort
  call clap#client#call_with_lnum('recent_files/on_move', function('clap#impl#on_move#handler'))
endfunction

function! s:recent_files.init() abort
  call clap#client#call_on_init('recent_files/on_init', function('clap#state#handle_response_on_typed'), {
        \ 'provider_id': g:clap.provider.id,
        \ 'query': has_key(g:clap.context, 'query') ? g:clap.context.query : g:clap.input.get(),
        \ 'source_fpath': expand('#'.g:clap.start.bufnr.':p'),
        \ 'cwd': clap#rooter#working_dir(),
        \ })
endfunction

let s:recent_files.sink = function('clap#provider#files#sink_impl')
let s:recent_files.enable_rooter = v:true
let s:recent_files.support_open_action = v:true
let s:recent_files.syntax = 'clap_files'

let g:clap#provider#recent_files# = s:recent_files

let &cpoptions = s:save_cpo
unlet s:save_cpo
