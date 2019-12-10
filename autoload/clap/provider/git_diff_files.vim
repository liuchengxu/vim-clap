" Author: KITAGAWA Yasutaka <kit494way@gmail.com>
" Description: List the files which is managed by git and have uncommitted changes.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:git_diff_files = {}

function! s:git_diff_files.source() abort
  if !executable('git')
    return ['git executable not found']
  endif

  redir => l:output
    silent execute '!git status -s -uno'
  redir END
  let l:output = filter(split(l:output, '\n')[1:], 'len(trim(v:val))')
  return map(l:output, 'split(v:val)[-1]')
endfunction

let s:git_diff_files.sink = 'e'
let s:git_diff_files.enable_rooter = v:true

let g:clap#provider#git_diff_files# = s:git_diff_files

let &cpoptions = s:save_cpo
unlet s:save_cpo
