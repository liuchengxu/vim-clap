" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Various preview support.

let s:save_cpo = &cpoptions
set cpoptions&vim

" Preview entry for files,history provider
function! clap#preview#file(fname) abort
  let fpath = expand(a:fname)
  if filereadable(fpath)
    let lines = readfile(fpath, '', 10)
    call g:clap.preview.show(lines)
    call g:clap.preview.set_syntax(clap#ext#into_filetype(a:fname))
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
