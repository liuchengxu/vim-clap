" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Jump to definition/reference based on the regexp.

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:dumb_jump = {}

function! s:dumb_jump.sink(selected) abort
  let pattern = '^\[\(\a\+\)\]\zs\(.*\):\(\d\+\):\(\d\+\):'
  let matched = matchlist(a:selected, pattern)
  let [fpath, linenr, column] = [matched[2], str2nr(matched[3]), str2nr(matched[4])]
  call clap#sink#open_file(fpath, linenr, column)
endfunction

function! s:into_qf_item(line) abort
  let pattern = '^\[\(\a\+\)\]\zs\(.*\):\(\d\+\):\(\d\+\):\(.*\)'
  let matched = matchlist(a:line, pattern)
  let [fpath, linenr, column, text] = [matched[2], str2nr(matched[3]), str2nr(matched[4]), matched[5]]
  return {'filename': fpath, 'lnum': linenr, 'col': column, 'text': text}
endfunction

function! s:dumb_jump_sink_star(lines) abort
  call clap#util#open_quickfix(map(a:lines, 's:into_qf_item(v:val)'))
endfunction

function! s:handle_response(result, error) abort
  if a:error isnot v:null
    call clap#indicator#set_matches_number(0)
    call g:clap.display.set_lines([a:error.message])
    return
  endif

  call clap#indicator#set_matches_number(a:result.total)

  if a:result.total == 0
    call g:clap.display.clear()
    call g:clap.preview.clear()
    return
  endif

  call g:clap.display.set_lines(a:result.lines)
  call clap#highlight#add_fuzzy_async_with_delay(a:result.indices)
  call clap#preview#async_open_with_delay()
endfunction

function! s:dumb_jump.on_typed() abort
  let extension = fnamemodify(bufname(g:clap.start.bufnr), ':e')
  call clap#client#call('dumb_jump/on_typed', function('s:handle_response'), {
        \ 'provider_id': g:clap.provider.id,
        \ 'query': g:clap.input.get(),
        \ 'input': g:clap.input.get(),
        \ 'extension': extension,
        \ 'cwd': clap#rooter#working_dir(),
        \ })
endfunction

function! s:dumb_jump.init() abort
  let extension = fnamemodify(bufname(g:clap.start.bufnr), ':e')
  call clap#client#call_on_init('dumb_jump/on_init', function('s:handle_response'), {
        \ 'provider_id': g:clap.provider.id,
        \ 'input': g:clap.input.get(),
        \ 'query': g:clap.input.get(),
        \ 'source_fpath': expand('#'.g:clap.start.bufnr.':p'),
        \ 'extension': extension,
        \ 'cwd': clap#rooter#working_dir(),
        \ })
endfunction

function! s:dumb_jump.on_move_async() abort
  call clap#client#call_on_move_dumb_jump('dumb_jump/on_move', function('clap#impl#on_move#handler'))
endfunction

let s:dumb_jump['sink*'] = function('s:dumb_jump_sink_star')
let s:dumb_jump.syntax = 'clap_dumb_jump'
let s:dumb_jump.enable_rooter = v:true
" let s:dumb_jump.on_move_async = function('clap#impl#on_move#async')
let g:clap#provider#dumb_jump# = s:dumb_jump

let &cpoptions = s:save_cpo
unlet s:save_cpo
