" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Get filetype based on the fname's extension.

let s:save_cpo = &cpoptions
set cpoptions&vim

" This is not complete, but should be enough to cover the most extensions.
" https://vi.stackexchange.com/questions/9962/get-filetype-by-extension-or-filename-in-vimscript
function! s:init_ext2ft() abort
  let matched = []
  for line in split(execute('autocmd filetypedetect'), "\n")
    if line =~? '\*\.\a\+\s*setf'
      call add(matched, line)
    endif
  endfor

  let s:ext2ft = {'h': 'c', 'hpp': 'cpp', 'vimrc': 'vim'}
  for line in matched
    let splitted = split(line)
    let ext = split(splitted[0], '\.')[-1]
    let ft = splitted[-1]
    let s:ext2ft[ext] = ft
  endfor
endfunction

if !exists('s:ext2ft')
  call s:init_ext2ft()
endif

function! clap#ext#into_filetype(fname) abort
  let ext = fnamemodify(a:fname, ':e')
  if !empty(ext) && has_key(s:ext2ft, ext)
    return s:ext2ft[ext]
  else
    return ''
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
