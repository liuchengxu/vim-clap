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

function! s:tags.on_move() abort
  try
    let [lnum, tag] = vista#finder#fzf#extract(g:clap.display.getcurline())
  catch
    return
  endtry
  let [start, end, hi_lnum] = clap#util#get_preview_line_range(lnum, 5)
  let lines = getbufline(g:clap.start.bufnr, start, end)
  call g:clap.preview.show(lines)
  call g:clap.preview.load_syntax(s:origin_ft)
  call g:clap.preview.add_highlight(hi_lnum+1)
endfunction

function! s:tags.on_enter() abort
  let s:origin_ft = getbufvar(g:clap.start.bufnr, '&ft')
  call g:clap.display.setbufvar('&ft', 'clap_tags')
endfunction

let s:tags.sink = function('vista#finder#fzf#sink')

let g:clap#provider#tags# = s:tags

let &cpoptions = s:save_cpo
unlet s:save_cpo
