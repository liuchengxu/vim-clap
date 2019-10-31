" Author: Ratheesh S<ratheeshreddy@gmail.com>
" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the registers

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:registers = {}

function! s:fetch_registers()
  let s = ''
  redir => s
  silent registers
  redir END
  return map(split(s, "\n")[1:], 'v:val[1:]')
endfunc

function! s:registers.source() abort
  return s:fetch_registers()
endfunction

function! s:registers.sink(line) abort
  execute 'normal! "'.split(a:line, ' ')[0].'p'
endfunction

let g:clap#provider#registers# = s:registers

let &cpoptions = s:save_cpo
unlet s:save_cpo
