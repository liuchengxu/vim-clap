" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the commits.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:commits = {}
let s:commits.syntax = 'clap_diff'
function! s:commits.source() abort
  let s:git_root = clap#path#get_git_root()
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
      call clap#helper#echo_error('The current buffer is not in the working tree')
      return []
    endif
    let source .= ' --follow '.current
  else
    let source .= ' --graph'
  endif
  return split(system(source), "\n")
endfunction

function! s:commits.sink(line) abort
endfunction

let s:begin = '^[^0-9]*[0-9]\{4}-[0-9]\{2}-[0-9]\{2}\s\+'
function! s:commits.on_move() abort
  let cur_line = g:clap.display.getcurline()
  let sha=matchstr(cur_line, s:begin.'\zs[a-f0-9]\+' )

  let gitshow = 'git show ' . sha
  let info = split(system(l:gitshow), '\n')
  if len(info) > 60
    let info = info[:60]
  endif

  call clap#preview#show_with_line_highlight(info, 'diff', len(info)+1)
  call clap#preview#highlight_header()
endfunction

function! s:commits.sink(line) abort
  let s:current = bufname(g:clap.start.bufnr)
  let sha=matchstr(a:line, s:begin.'\zs[a-f0-9]\+' )
  let gitshow = '!git show ' .  sha
  vertical botright new
  setlocal buftype=nofile bufhidden=wipe noswapfile nomodeline

  setlocal modifiable
  silent execute 'read' escape(gitshow, '%')
  normal! gg"_dd
  setfiletype diff
  setlocal nomodifiable
endfunction

let s:commits.syntax = 'clap_commits'

let g:clap#provider#commits# = s:commits

let &cpoptions = s:save_cpo
unlet s:save_cpo
