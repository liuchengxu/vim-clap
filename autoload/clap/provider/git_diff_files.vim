" Author: KITAGAWA Yasutaka <kit494way@gmail.com>
" Description: List the files which is managed by git and have uncommitted changes.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:git_diff_files = {}

function! s:git_diff_files.source() abort
  if !executable('git')
    call clap#helper#echo_error('git executable not found')
    return []
  endif

  let changed = systemlist('git status -s -uno')
  if v:shell_error
    call clap#helper#echo_error('Error occurs on calling `git status -s -uno`, maybe you are not in a git repo.')
    return []
  else
    return map(changed, 'split(v:val)[-1]')
  endif
endfunction

function! s:git_diff_files_on_move() abort
  let diff = 'git --no-pager diff -U0'
  let pipetail = '| tail -n +5'
  let filediff = g:clap.display.getcurline().pipetail
  let command = systemlist(diff.' '.filediff)

  if !empty(command)
    if command[0]=~#"^fatal"
      let command = systemlist(diff.' -- '.filediff)
      if empty(command)
        let command = systemlist(diff.' --cached -- '.filediff)
      endif
    endif
  else
    let command = systemlist(diff.' --cached '.filediff)
  endif

  if !empty(command)
    call g:clap.preview.show(command[:10])
  else
    call g:clap.preview.show([''])
  endif
  call g:clap.preview.set_syntax('diff')
endfunction

let s:git_diff_files.sink = 'e'
let s:git_diff_files.enable_rooter = v:true
let s:git_diff_files.on_move = function('s:git_diff_files_on_move')

let g:clap#provider#git_diff_files# = s:git_diff_files

let &cpoptions = s:save_cpo
unlet s:save_cpo
