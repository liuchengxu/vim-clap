" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the marks.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:marks = {}

function! s:format_mark(line) abort
  return substitute(a:line, '\S', '\=submatch(0)', '')
endfunction

function! s:marks.source() abort
  call g:clap.start.goto_win()
  let cout = execute('marks')
  call g:clap.input.goto_win()
  let list = split(cout, "\n")
  return extend(list[0:0], map(list[1:], 's:format_mark(v:val)'))
endfunction

function! s:marks.sink(line) abort
  execute 'normal! `'.matchstr(a:line, '\S').'zz'
endfunction

function! clap#provider#marks#preview_impl(line, col, file_text) abort
  let [line, col, file_text] = [a:line, a:col, a:file_text]

  let origin_line = getbufline(g:clap.start.bufnr, line)

  let [start, end, hi_lnum] = clap#preview#get_range(line)

  let should_add_hi = v:true

  " file_text is the origin line with leading white spaces trimmed.
  if !empty(origin_line)
        \ && clap#util#trim_leading(origin_line[0]) == file_text
    let lines = getbufline(g:clap.start.bufnr, start, end)
    call insert(lines, bufname(g:clap.start.bufnr))
    let l:preview_header_added = 1
    let hi_lnum += 1
    let origin_bufnr = g:clap.start.bufnr
  else
    " TODO try cwd + file_text
    if filereadable(expand(file_text))
      let lines = readfile(expand(file_text), '', end)[start :]
      call insert(lines, file_text)
      let l:preview_header_added = 1
    else
      let lines = [file_text]
      let should_add_hi = v:false
    endif
  endif

  if empty(lines)
    return
  endif

  call g:clap.preview.show(lines)

  if should_add_hi
    if exists('l:origin_bufnr')
      let ft = getbufvar(l:origin_bufnr, '&filetype')
      if empty(ft)
        let ft = fnamemodify(expand(bufname(origin_bufnr)), ':e')
      endif
    else
      let ft = clap#ext#into_filetype(file_text)
    endif
    if !empty(ft)
      call g:clap.preview.set_syntax(ft)
    endif
    if exists('l:preview_header_added')
      let hi_lnum += 1
      call clap#preview#highlight_header()
    endif
    call g:clap.preview.add_highlight(hi_lnum)
  endif
endfunction

function! s:marks.on_move() abort
  let curline = g:clap.display.getcurline()

  if 'mark line  col file/text' ==# curline
    return
  endif

  let matched = matchlist(curline, '^.*\([a-zA-Z0-9[`''"\^\]\.]\)\s\+\(\d\+\)\s\+\(\d\+\)\s\+\(.*\)$')

  if len(matched) < 5
    return
  endif

  let line = matched[2]
  let col = matched[3]
  let file_text = matched[4]

  call clap#provider#marks#preview_impl(line, col, file_text)
endfunction

let s:marks.syntax = 'clap_marks'

let g:clap#provider#marks# = s:marks

let &cpoptions = s:save_cpo
unlet s:save_cpo
