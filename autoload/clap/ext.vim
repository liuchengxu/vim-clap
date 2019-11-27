" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Get filetype based on the fname's extension.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:ext_to_ft = {
      \ 'rs': 'rust',
      \ 'js': 'javascript',
      \ 'vim': 'vim',
      \ 'md': 'markdown',
      \ }

function! clap#ext#into_filetype(fname) abort
  let ext = fnamemodify(a:fname, ':e')
  if !empty(ext) && has_key(s:ext_to_ft, ext)
    return s:ext_to_ft[ext]
  else
    return ''
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
