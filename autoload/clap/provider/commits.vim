" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the commits.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:commits = {}

function! s:commits.source() abort
  let s:git_root = clap#util#get_git_root()
  if empty(s:git_root)
    call g:clap.abort('Not in git repository')
    return
  endif

  let source = 'git log ''--color=never'' ''--date=short'' ''--format=%cd %h%d %s (%an)'' --graph'
  let current = bufname(g:clap.start.bufnr)
  let managed = 0
  if !empty(current)
    call system('git show '.current.' 2> '.(has('win32') ? 'nul' : '/dev/null'))
    let managed = !v:shell_error
  endif

  let buffer_local = 0
  if buffer_local
    if !managed
      call clap#error('The current buffer is not in the working tree')
      return []
    endif
    let source .= ' --follow '.current
  else
    let source .= ' --graph'
  endif
  return split(system(source), "\n")
endfunction

function! s:commits.sink(line) abort
  call g:clap.abort('Not implemented yet')
endfunction

let s:commits.on_enter = { -> g:clap.display.setbufvar('&ft', 'clap_commits') }

let g:clap#provider#commits# = s:commits

let &cpoptions = s:save_cpo
unlet s:save_cpo
