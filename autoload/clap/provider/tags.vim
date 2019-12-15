" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the tags based on vista.vim.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:tags = {}

function! s:tags.source(...) abort
  if !exists('t:vista')
    let [bufnr, winnr, fname, fpath] = [
          \ g:clap.start.bufnr,
          \ g:clap.start.winid,
          \ bufname(g:clap.start.bufnr),
          \ expand('#'.g:clap.start.bufnr.':p')
          \ ]
    call vista#source#Update(bufnr, winnr, fname, fpath)
  endif

  let [data, _, _] = call('vista#finder#GetSymbols', a:000)

  if empty(data)
    return ['No symbols found via vista.vim']
  endif

  return vista#finder#PrepareSource(data)
endfunction

let s:tags.sink = function('vista#finder#fzf#sink')
let s:tags.on_enter = { -> g:clap.display.setbufvar('&ft', 'clap_tags') }

let g:clap#provider#tags# = s:tags

let &cpoptions = s:save_cpo
unlet s:save_cpo
