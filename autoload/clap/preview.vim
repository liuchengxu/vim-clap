" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Various preview support.

let s:save_cpo = &cpoptions
set cpoptions&vim

" Preview entry for files,history provider
function! clap#preview#file(fname) abort
  let fpath = expand(a:fname)
  if filereadable(fpath)
    let lines = readfile(fpath, '', 10)
    call insert(lines, fpath)
    call g:clap.preview.show(lines)
    call g:clap.preview.set_syntax(clap#ext#into_filetype(a:fname))
    call clap#preview#highlight_header()
  endif
endfunction

" Sometime the first line of preview window is used for the header.
function! clap#preview#highlight_header() abort
  try
    let winid = win_getid()
    call g:clap.preview.goto_win()
    if !exists('w:preview_header_id')
      let w:preview_header_id = matchaddpos('Title', [1])
    endif
  finally
    noautocmd call win_gotoid(winid)
  endtry
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
