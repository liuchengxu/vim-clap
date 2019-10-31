" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the register list.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:registers = {}

" Credit: https://github.com/junegunn/vim-peekaboo
function! s:append_group(title, regs)
  call add(s:lines, a:title.':')
  for r in a:regs
    let val = eval('@'.r)[:&columns]
    if !empty(val)
      call add(s:lines, printf(' %s: %s', r, val))
    endif
  endfor
endfunction

function! s:registers.source() abort
  let s:lines = []
  call s:append_group('Special', ['"', '*', '+', '-'])
  call add(s:lines, '')
  call s:append_group('Last-Search-Pattern', ['/'])
  call add(s:lines, '')
  call s:append_group('Read-only', ['.', ':'])
  call add(s:lines, '')
  call s:append_group('Numbered', map(range(0, 9), 'string(v:val)'))
  call add(s:lines, '')
  call s:append_group('Named', map(range(97, 97 + 25), 'nr2char(v:val)'))
  return s:lines
endfunction

function! s:registers.sink(selected) abort
  let reg = matchstr(a:selected, '^\s*\zs\(.\)\ze: ')
  execute 'normal!' '"'.reg.'p'
endfunction

let s:registers.on_enter = { -> g:clap.display.setbufvar('&ft', 'clap_registers') }

let g:clap#provider#registers# = s:registers

let &cpoptions = s:save_cpo
unlet s:save_cpo
