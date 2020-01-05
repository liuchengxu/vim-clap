" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the files managed by git.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:git_files = {}

function! s:git_files.source() abort
  if !executable('git')
    call clap#helper#echo_error('git executable not found')
    return []
  endif

  let changed = systemlist('git ls-files '.(has('win32') ? '' : ' | uniq'))
  if v:shell_error
    call clap#helper#echo_error('Error occurs on calling `git ls-files`. Maybe you are not in a git repo?')
    return []
  else
    return map(changed, 'split(v:val)[-1]')
  endif
endfunction

function! s:git_files.sink(selected) abort
  if has_key(g:clap, 'open_action')
    execute g:clap.open_action a:selected
  else
    execute 'edit' a:selected
  endif
endfunction

let s:git_files.enable_rooter = v:true
let s:git_files.support_open_action = v:true
let s:git_files.on_enter = { -> g:clap.display.setbufvar('&syntax', 'clap_files')}

let g:clap#provider#git_files# = s:git_files

let &cpoptions = s:save_cpo
unlet s:save_cpo
