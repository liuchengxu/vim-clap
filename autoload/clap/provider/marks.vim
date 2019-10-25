" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the marks.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:marks = {}

let s:ext_to_ft = {'rs': 'rust', 'js': 'javascript'}

let s:preview_size = 5

function! s:format_mark(line) abort
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

if has('nvim')
  function! s:execute_matchaddpos(lnum) abort
    noautocmd call win_gotoid(g:clap.preview.winid)
    call s:matchaddpos(a:lnum)
    noautocmd call win_gotoid(g:clap.input.winid)
  endfunction

  function! s:render_syntax(ft) abort
    call g:clap.preview.setbufvar('&ft', a:ft)
  endfunction
else
  function! s:execute_matchaddpos(lnum) abort
    call win_execute(g:clap.preview.winid, 'noautocmd call s:matchaddpos(a:lnum)')
  endfunction

  function! s:render_syntax(ft) abort
    " vim using noautocmd in win_execute, hence we have to load the syntax file manually.
    call win_execute(g:clap.preview.winid, 'runtime syntax/'.a:ft.'.vim')
  endfunction
endif

function! clap#provider#marks#preview_impl(line, col, file_text) abort
  let [line, col, file_text] = [a:line, a:col, a:file_text]

  let origin_line = getbufline(g:clap.start.bufnr, line)

  let [start, end, hi_lnum] = clap#util#get_preview_line_range(line, s:preview_size)

  let should_add_hi = v:true

  " file_text is the origin line with leading white spaces trimmed.
  if !empty(origin_line)
        \ && clap#util#trim_leading(origin_line[0]) == file_text
    let lines = getbufline(g:clap.start.bufnr, start, end)
    let hi_lnum += 1
    let origin_bufnr = g:clap.start.bufnr
  else
    " TODO try cwd + file_text
    if filereadable(expand(file_text))
      let lines = readfile(expand(file_text), '', end)[start :]
    else
      let lines = [file_text]
      let should_add_hi = v:false
    endif
  endif

  call g:clap.preview.show(lines)

  if should_add_hi
    if exists('l:origin_bufnr')
      let ft = getbufvar(l:origin_bufnr, '&filetype')
      if empty(ft)
        let ft = fnamemodify(expand(bufname(origin_bufnr)), ':e')
      endif
    else
      let ext = fnamemodify(file_text, ':e')
      if !empty(ft) && has_key(s:ext_to_ft, ext)
        let ft = s:ext_to_ft[ext]
      else
        let ft = ''
      endif
    endif
    if !empty(ft)
      call s:render_syntax(ft)
    endif
    call s:execute_matchaddpos(hi_lnum)
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

let s:marks.on_enter = { -> g:clap.display.setbufvar('&ft', 'clap_marks') }

let g:clap#provider#marks# = s:marks

let &cpoptions = s:save_cpo
unlet s:save_cpo
