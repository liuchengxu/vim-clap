" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the commits.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:begin = '^[^0-9]*[0-9]\{4}-[0-9]\{2}-[0-9]\{2}\s\+'

let s:commits = {}

function! clap#provider#commits#source_common(buffer_local) abort
  let git_root = clap#path#get_git_root()
  if empty(git_root)
    return ['Not in git repository']
  endif

  let source = "git log '--color=never' '--date=short' '--format=%cd %h%d %s (%an)'"

  let current = bufname(g:clap.start.bufnr)
  if empty(current)
    return ['buffer name is empty']
  endif

  call system('git show '.current.' 2> '.(has('win32') ? 'nul' : '/dev/null'))

  if v:shell_error
    return ['The current buffer is not in the working tree']
  endif

  if a:buffer_local
    return source." '--follow' '--' ".current
  else
    return source.' --graph'
  endif
endfunction

function! s:commits.source() abort
  return clap#provider#commits#source_common(v:false)
endfunction

function! clap#provider#commits#on_move_common(cmd) abort
  let lines = systemlist(a:cmd)
  let lines = lines[:60]
  call clap#preview#show_lines(lines, 'diff', -1)
  call clap#preview#highlight_header()
endfunction

function! clap#provider#commits#parse_rev(line) abort
  return matchstr(a:line, s:begin.'\zs[a-f0-9]\+')
endfunction

function! s:commits.on_move() abort
  let cur_line = g:clap.display.getcurline()
  let rev = clap#provider#commits#parse_rev(cur_line)
  call clap#provider#commits#on_move_common('git show '.rev)
endfunction

function! clap#provider#commits#on_move_callback(result, error) abort
  if a:error isnot v:null
    return
  endif
  let lines = a:result.lines
  call clap#preview#show_lines(lines, 'diff', -1)
  call clap#preview#highlight_header()
endfunction

function! s:commits.on_move_async() abort
  call clap#client#call_on_move('on_move', function('clap#provider#commits#on_move_callback'))
endfunction

function! clap#provider#commits#sink_inner(bang_cmd) abort
  vertical botright new
  setlocal buftype=nofile bufhidden=wipe noswapfile nomodeline

  setlocal modifiable
  silent execute 'read' escape(a:bang_cmd, '%')
  normal! gg"_dd
  setfiletype diff
  setlocal nomodifiable
endfunction

function! s:commits.sink(line) abort
  let rev = clap#provider#commits#parse_rev(a:line)
  call clap#provider#commits#sink_inner('!git show '.rev)
endfunction

let s:commits.syntax = 'clap_diff'

let g:clap#provider#commits# = s:commits

let &cpoptions = s:save_cpo
unlet s:save_cpo
