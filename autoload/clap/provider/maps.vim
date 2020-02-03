" Author: Mark Wu <markplace@gmail.com>
" Description: List the maps.

let s:save_cpo = &cpoptions
set cpoptions&vim

" Derived from fzf.vim
let s:allowed_mode = ['n', 'i', 'x', 'o']

function! s:align_pairs(list) abort
  let maxlen = 0
  let pairs = []
  for elem in a:list
    let match = matchlist(elem, '^\(\S*\)\s*\(.*\)$')
    let [_, k, v] = match[0:2]
    let maxlen = max([maxlen, len(k)])
    call add(pairs, [k, substitute(v, '^\*\?[@ ]\?', '', '')])
  endfor
  let maxlen = min([maxlen, 35])
  return map(pairs, "printf('%-'.maxlen.'s', v:val[0]).' '.v:val[1]")
endfunction

function! s:maps_source() abort
  let mode = get(g:clap.context, 'mode', 'n')
  if index(s:allowed_mode, mode) == -1
    let mode = 'n'
  endif

  let s:map_gv  = mode ==# 'x' ? 'gv' : ''
  let s:map_cnt = v:count == 0 ? '' : v:count
  let s:map_reg = empty(v:register) ? '' : ('"'.v:register)
  let s:map_op  = mode ==# 'o' ? v:operator : ''

  redir => cout
  silent execute 'verbose' mode.'map'
  redir END
  let list = []
  let curr = ''
  for line in split(cout, "\n")
    if line =~# "^\t"
      let src = '  '.join(reverse(reverse(split(split(line)[-1], '/'))[0:2]), '/')
      call add(list, printf('%s %s', curr, src))
      let curr = ''
    else
      let curr = line[3:]
    endif
  endfor
  if !empty(curr)
    call add(list, curr)
  endif
  return sort(s:align_pairs(list))
endfunction

function! s:maps_sink(selected) abort
  let key = matchstr(a:selected, '^\S*')
  redraw
  call feedkeys(s:map_gv.s:map_cnt.s:map_reg, 'n')
  call feedkeys(s:map_op.
        \ substitute(key, '<[^ >]\+>', '\=eval("\"\\".submatch(0)."\"")', 'g'))
endfunction

let s:maps = {}
let s:maps.sink = function('s:maps_sink')
let s:maps.source = function('s:maps_source')

let g:clap#provider#maps# = s:maps

let &cpoptions = s:save_cpo
unlet s:save_cpo
