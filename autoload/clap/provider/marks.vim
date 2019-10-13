" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the marks.

let s:save_cpo = &cpo
set cpo&vim

let s:marks = {}

function! s:format_mark(line)
  return substitute(a:line, '\S', '\=submatch(0)', '')
endfunction

function! s:marks.source() abort
  call g:clap.start.goto_win()
  redir => cout
  silent marks
  redir END
  call g:clap.input.goto_win()
  let list = split(cout, "\n")
  return extend(list[0:0], map(list[1:], 's:format_mark(v:val)'))
endfunction

function! s:marks.sink(line) abort
  execute 'normal! `'.matchstr(a:line, '\S').'zz'
endfunction

function! s:matchaddpos(lnum) abort
  if exists('w:clap_mark_hi_id')
    call matchdelete(w:clap_mark_hi_id)
  endif
  let w:clap_mark_hi_id = matchaddpos('Search', [[a:lnum]])
endfunction

function! s:marks.on_move() abort
  let curline = g:clap.display.getcurline()

  if 'mark line  col file/text' == curline
    return
  endif

  let matched = matchlist(curline, '^.*\([a-zA-Z0-9[`''"\^\]\.]\)\s\+\(\d\+\)\s\+\(\d\+\)\s\+\(.*\)$')
  let line = matched[2]
  let col = matched[3]
  let file_text = matched[4]

  let origin_line = getbufline(g:clap.start.bufnr, line)

  if line - 5 > 0
    let start = line - 5
    let match_start = 5+1
  else
    let start = line
    let match_start = line
  endif

  let l:match_hi = v:true
  " file_text is the origin line with leading white spaces trimmed.
  if !empty(origin_line) && clap#util#trim_leading(origin_line[0]) == file_text

    let lines = getbufline(g:clap.start.bufnr, start, line+5)

  elseif filereadable(expand(file_text))

    let bufnr = bufadd(file_text)

    if !bufloaded(bufnr)
      silent call bufload(bufnr)
    endif

    let lines = getbufline(bufnr, start, line+5)

  else
    let lines = [file_text]
    let l:match_hi = v:false
  endif

  call g:clap.preview.show(lines)

  if l:match_hi
    if has('nvim')
      noautocmd call win_gotoid(g:clap.preview.winid)
      call s:matchaddpos(match_start)
      noautocmd call win_gotoid(g:clap.input.winid)
    else
      " Too many plugins use redir, so we can't add highlight for vim for now.
      " E930: Cannot use :redir inside execute()
      " call win_execute(g:clap.preview.winid, "call s:matchaddpos(l:match_start)")
    endif
  endif
endfunction

let s:marks.on_enter = { -> g:clap.display.setbufvar('&ft', 'clap_marks') }

let g:clap#provider#marks# = s:marks

let &cpo = s:save_cpo
unlet s:save_cpo
