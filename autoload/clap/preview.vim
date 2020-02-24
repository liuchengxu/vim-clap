" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Various preview support.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:path_seperator = has('win32') ? '\' : '/'

function! s:peek_file(fname, fpath) abort
  let lines = readfile(a:fpath, '', 10)
  call insert(lines, a:fpath)
  call g:clap.preview.show(lines)
  call g:clap.preview.set_syntax(clap#ext#into_filetype(a:fname))
  call clap#preview#highlight_header()
endfunction

function! s:show_file_props(entry) abort
  let props = strftime('%B %d/%m/%Y %H:%M:%S', getftime(a:entry)).'    '.getfperm(a:entry)
  call g:clap.preview.show([a:entry, props])
  call clap#preview#highlight_header()
endfunction

" Preview entry for files,history provider
function! clap#preview#file(fname) abort
  let fpath = expand(a:fname)
  if filereadable(fpath)
    call s:peek_file(a:fname, fpath)
    return
  elseif exists('g:__clap_provider_cwd')
    let fpath_with_cwd = g:__clap_provider_cwd.s:path_seperator.fpath
    if filereadable(fpath_with_cwd)
      call s:peek_file(a:fname, fpath_with_cwd)
      return
    endif
  endif
  call s:show_file_props(a:fname)
endfunction

" Given the origin lnum and the size of range, return
" [origin_lnum-range_size, origin_lnum+range_size] and the target lnum that
" the origin line should be positioned.
" 0-based
function! clap#preview#get_line_range(origin_lnum, range_size) abort
  if a:origin_lnum - a:range_size > 0
    return [a:origin_lnum - a:range_size, a:origin_lnum + a:range_size, a:range_size]
  else
    return [0, a:origin_lnum + a:range_size, a:origin_lnum]
  endif
endfunction

function! clap#preview#show_with_line_highlight(lines, syntax, hi_lnum) abort
  call g:clap.preview.show(a:lines)
  call g:clap.preview.set_syntax(a:syntax)
  if a:hi_lnum > 0
    call g:clap.preview.add_highlight(a:hi_lnum)
  endif
endfunction

function! s:highlight_header() abort
  if !exists('w:preview_header_id')
    let w:preview_header_id = matchaddpos('Title', [1])
  endif
endfunction

if has('nvim')
  " Sometime the first line of preview window is used for the header.
  function! clap#preview#highlight_header() abort
    try
      let winid = win_getid()
      call g:clap.preview.goto_win()
      call s:highlight_header()
    finally
      noautocmd call win_gotoid(winid)
    endtry
  endfunction
else
  function! clap#preview#highlight_header() abort
    call win_execute(g:clap.preview.winid, 'noautocmd call s:highlight_header()')
  endfunction
endif

let &cpoptions = s:save_cpo
unlet s:save_cpo
