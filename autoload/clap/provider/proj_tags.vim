" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Project-wise tags

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:proj_tags = {}
let s:PATH_SEPERATOR = has('win32') ? '\' : '/'

function! s:proj_tags.on_typed() abort
  if exists('g:__clap_forerunner_tempfile')
    call clap#filter#async#dyn#from_tempfile(g:__clap_forerunner_tempfile)
  elseif exists('g:__clap_forerunner_result')
    let query = g:clap.input.get()
    if query ==# ''
      return
    endif
    call clap#filter#on_typed(function('clap#filter#sync'), query, g:__clap_forerunner_result)
  else
    let cmd = clap#maple#build_cmd(printf('tags "%s" "%s"', g:clap.input.get(), clap#rooter#working_dir()))
    call clap#filter#async#dyn#start_directly(cmd)
  endif
endfunction

function! s:proj_tags.init() abort
  let g:__clap_builtin_content_filtering_enum = 'TagNameOnly'
  if clap#maple#is_available()
    call clap#rooter#try_set_cwd()
    call clap#forerunner#start_subcommand(clap#maple#tags_forerunner_subcommand())
  endif
endfunction

function! s:proj_tags.sink(selected) abort
  let lnum = matchstr(a:selected, '^.*:\zs\(\d\+\)')
  let path = matchstr(a:selected, '\t\zs\f*$')

  normal! m'

  if has_key(g:clap, 'open_action')
    execute g:clap.open_action path
  else
    " Cannot use noautocmd here as it would lose syntax, and ...
    execute 'edit' path
  endif

  call cursor(lnum, 1)
  normal! zz
endfunction

function! s:proj_tags.on_move() abort
  let curline = g:clap.display.getcurline()
  let lnum = matchstr(curline, '^.*:\zs\(\d\+\)')
  let path = matchstr(curline, '\t\zs\f*$')
  let [start, end, hi_lnum] = clap#preview#get_line_range(lnum, 5)
  if filereadable(path)
    let lines = readfile(path)[start : end]
  else
    let cwd = clap#rooter#working_dir()
    if filereadable(cwd.s:PATH_SEPERATOR.path)
      let lines = readfile(cwd.s:PATH_SEPERATOR.path)[start : end]
    else
      return
    endif
  endif
  call insert(lines, path)
  call g:clap.preview.show(lines)
  call g:clap.preview.set_syntax(clap#ext#into_filetype(path))
  call g:clap.preview.add_highlight(hi_lnum+1)
  call clap#preview#highlight_header()
endfunction

function! s:proj_tags.on_exit() abort
  if exists('g:__clap_builtin_content_filtering_enum')
    unlet g:__clap_builtin_content_filtering_enum
  endif
endfunction

let s:proj_tags.enable_rooter = v:true
let s:proj_tags.support_open_action = v:true
let s:proj_tags.syntax = 'clap_tags'

let g:clap#provider#proj_tags# = s:proj_tags

let &cpoptions = s:save_cpo
unlet s:save_cpo
