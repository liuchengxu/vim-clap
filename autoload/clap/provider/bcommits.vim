" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the buffer commits.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:bcommits = {}
let s:bcommits.syntax = 'clap_diff'

let s:current = ''
let s:begin = '^[^0-9]*[0-9]\{4}-[0-9]\{2}-[0-9]\{2}\s\+'
let s:shas = []

function! s:bcommits.source() abort
  let s:shas = split(system('git log --format=format:%h'), "\n")
  let s:git_root = clap#path#get_git_root()
  if empty(s:git_root)
    call g:clap.abort('Not in git repository')
    return
  endif

  let s:current = bufname(g:clap.start.bufnr)
  if empty(s:current)
    return ['The current buffer is not in the working tree' . s:current]
  else
    call system('git show '.s:current.' 2> '.(has('win32') ? 'nul' : '/dev/null'))
  endif
  let s:source = "git log '--color=never' '--date=short' '--format=%cd %h%d %s (%an)' '--follow' '--' ".s:current
  return s:source
endfunction


function! s:bcommits.on_move() abort
  let cur_line = g:clap.display.getcurline()
  let sha = matchstr(cur_line, s:begin.'\zs[a-f0-9]\+' )

  let prev = s:find_prev(sha)
  let gitdiff = 'git diff --color=never ' . sha . ' ' . prev . ' -- ' . ' '.s:current
  let info = split(system(l:gitdiff), '\n')
  if len(info) > 60
    let info = info[:60]
  endif

  call clap#preview#show_with_line_highlight(info, 'diff', len(info)+1)
  call clap#preview#highlight_header()
endfunction

function! s:bcommits.sink(line) abort
  let sha = matchstr(a:line, s:begin.'\zs[a-f0-9]\+' )
  let prev = s:find_prev(sha)
  let cmd = '!git diff --color=never '.sha.' '.prev.' -- '.bufname(g:clap.start.bufnr)
  call clap#provider#commits#sink_inner(cmd)
endfunction

function! s:find_prev(ver) abort
  if len(s:shas) <= 0
    let s:shas = split(system('git log --format=format:%h'), "\n")
  endif
  let idx = 0
  let prev = 'master'
  for commit in s:shas
    if commit == a:ver
      if idx + 1 < len(s:shas)
        let prev = s:shas[idx+1]
      endif
      return prev
    endif
    let idx = idx+1
  endfor
  return prev
endfunction

let g:clap#provider#bcommits# = s:bcommits

let &cpoptions = s:save_cpo
unlet s:save_cpo
