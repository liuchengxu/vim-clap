" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the buffer lines.

let s:save_cpo = &cpo
set cpo&vim

let s:blines = {}

function! s:blines.sink(selected) abort
  let lnum = matchstr(a:selected, '^\s*\(\d\+\) ')
  let lnum = str2nr(trim(lnum))
  call g:clap.start.goto_win()
  silent call cursor(lnum, 1)
  normal! ^zvzz
endfunction

function! s:blines.source() abort
  let lines = g:clap.start.get_lines()
  let linefmt = '%4d %s'
  return map(lines, 'printf(linefmt, v:key + 1, v:val)')
endfunction

function! s:blines.source_async() abort
  let lines = self.source()
  let tmp = tempname()
  if writefile(lines, tmp) == 0
    let l:cur_input = g:clap.input.get()
    let ext_filter_cmd = clap#filter#get_external_cmd_or_default()
    let cmd = printf('cat %s | %s', tmp, ext_filter_cmd)
    call add(s:tmps, tmp)
    return cmd
  else
    call g:clap.abort("Fail to write source to a temp file")
  endif
endfunction

function! s:blines.on_enter() abort
  let s:tmps = []
  call g:clap.display.setbufvar('&ft', 'clap_blines')
endfunction

function! s:blines.on_exit() abort
  call map(s:tmps, 'delete(v:val)')
endfunction

let g:clap#provider#blines# = s:blines

let &cpo = s:save_cpo
unlet s:save_cpo
