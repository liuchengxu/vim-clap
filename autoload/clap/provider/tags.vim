" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the tags based on vista.vim.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:tags = {}

function! s:tags.source(...) abort
  let [bufnr, winnr, fname, fpath] = [
        \ g:clap.start.bufnr,
        \ win_id2win(g:clap.start.winid),
        \ bufname(g:clap.start.bufnr),
        \ expand('#'.g:clap.start.bufnr.':p')
        \ ]

  try
    call vista#source#Update(bufnr, winnr, fname, fpath)
  catch
    return [v:exception, 'Ensure you have installed https://github.com/liuchengxu/vista.vim']
  endtry

  if len(g:clap.provider.args) == 1 && index(g:vista#executives, g:clap.provider.args[0]) > -1
    let executive = g:clap.provider.args
  else
    let executive = []
  endif

  let [data, cur_executive, using_alternative] = call('vista#finder#GetSymbols', executive)

  if empty(data)
    return ['No symbols found via vista.vim']
  endif

  if using_alternative
    let self.prompt_format = ' %spinner%%forerunner_status%*'.cur_executive.':'
  else
    let self.prompt_format = ' %spinner%%forerunner_status%'.cur_executive.':'
  endif
  call clap#spinner#refresh()

  return vista#finder#PrepareSource(data)
endfunction

function! s:tags.on_move() abort
  try
    let [lnum, tag] = vista#finder#fzf#extract(g:clap.display.getcurline())
  catch
    return
  endtry
  call clap#preview#buffer(lnum, s:origin_syntax)
endfunction

function! s:tags.on_enter() abort
  let s:origin_syntax = getbufvar(g:clap.start.bufnr, '&syntax')
  call g:clap.display.setbufvar('&syntax', 'clap_tags')
  let g:__clap_match_type_enum = 'TagName'
endfunction

function! s:tags.on_exit() abort
  if exists('g:__clap_match_type_enum')
    unlet g:__clap_match_type_enum
  endif
endfunction

function! s:tags.sink(selected) abort
  call vista#finder#fzf#sink(a:selected, g:clap.start.winid)
endfunction

let s:tags.on_move_async = function('clap#impl#on_move#async')

let g:clap#provider#tags# = s:tags

let &cpoptions = s:save_cpo
unlet s:save_cpo
