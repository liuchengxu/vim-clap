" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the entries of the location list.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:loclist = {}

function! s:loclist.source() abort
  let loclist = getloclist(g:clap.start.winid)
  if empty(loclist)
    return ['Location list is empty for window '.g:clap.start.winid]
  else
    " User can narrow down the result list, thus we note the original loclist index ahead.
    let s:locline2idx = {}
    return map(loclist, 's:into_loc_line(v:key, v:val)')
  endif
endfunction

function! s:into_loc_line(idx, loc_entry) abort
  let loc_line = clap#provider#quickfix#into_qf_line(a:loc_entry)
  let s:locline2idx[loc_line] = a:idx
  return loc_line
endfunction

function! s:loclist.sink(selected) abort
  let [_, lnum, column] = clap#provider#quickfix#extract_position(a:selected)
  call cursor(lnum, column)
endfunction

" Split a very long line into serveral shorter lines.
function! s:equant(long_line, width) abort
  let idx = 0
  let lines = []
  while idx * a:width < strlen(a:long_line)
    let start = idx * a:width
    let end = (idx+1) * a:width
    call add(lines, a:long_line[start : end - 1])
    let idx += 1
  endwhile
  return lines
endfunction

function! s:loclist.on_move() abort
  let locations = getloclist(g:clap.start.winid)
  if empty(locations)
    return
  endif

  let curline = g:clap.display.getcurline()
  let winwidth = winwidth(g:clap.display.winid)

  let locitem = locations[str2nr(s:locline2idx[curline])]

  let lines = []
  call add(lines, '--> '.bufname(locitem.bufnr).':'.locitem.lnum.':'.locitem.col)

  if locitem.lnum !=# ''
    let line = getbufline(locitem.bufnr, str2nr(locitem.lnum))
    if !empty(line)
      call add(lines, line[0])
      if locitem.col !=# ''
        call add(lines, repeat(' ', locitem.col - 1).'^')
      endif
    endif
  endif

  " The text may have multiple lines.
  let items = split(locitem.text, "\n")
  for item in items
    if strlen(item) > winwidth
      call extend(lines, s:equant(item, winwidth))
    else
      call add(lines, item)
    endif
  endfor

  call g:clap.preview.show(lines)
  call g:clap.preview.setbufvar('&syntax', getbufvar(locitem.bufnr, '&syntax'))
endfunction

let s:loclist.syntax = 'qf'
let g:clap#provider#loclist# = s:loclist

let &cpoptions = s:save_cpo
unlet s:save_cpo
