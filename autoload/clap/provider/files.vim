" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the files.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:files = {}

let s:default_opts = {
      \ 'fd': '--type f',
      \ 'rg': '--files',
      \ 'git': 'ls-tree -r --name-only HEAD',
      \ 'find': '. -type f',
      \ }

let s:default_finder = v:null

for exe in ['fd', 'rg', 'git', 'find']
  if executable(exe)
    let s:default_finder = exe
    break
  endif
endfor

if s:default_finder is v:null
  let s:default_source = ['No usable tools found for the files provider']
else
  let s:default_source = join([s:default_finder, s:default_opts[s:default_finder]], ' ')
endif

function! s:files.source() abort
  call clap#rooter#try_set_cwd()

  if has_key(g:clap.context, 'finder')
    let finder = g:clap.context.finder
    return finder.' '.join(g:clap.provider.args, ' ')
  elseif g:clap.provider.args == ['--hidden']
    if s:default_finder ==# 'fd' || s:default_finder ==# 'rg'
      return join([s:default_finder, s:default_opts[s:default_finder], '--hidden'], ' ')
    else
      return s:default_source
    endif
  else
    return s:default_source
  endif
endfunction

function! s:into_filename(line) abort
  if g:clap_enable_icon && clap#maple#is_available()
    return a:line[4:]
  else
    return a:line
  endif
endfunction

function! clap#provider#files#sink_impl(selected) abort
  let fpath = s:into_filename(a:selected)

  if has_key(g:clap, 'open_action')
    execute g:clap.open_action fpath
  else
    execute 'edit' fpath
  endif
endfunction

function! clap#provider#files#sink_star_impl(lines) abort
  call clap#util#open_quickfix(map(map(a:lines, 's:into_filename(v:val)'),
        \ '{'.
        \   '"filename": v:val,'.
        \   '"text": strftime("Modified %b,%d %Y %H:%M:%S", getftime(v:val))." ".getfperm(v:val)'.
        \ '}'))
endfunction

function! clap#provider#files#on_move_impl() abort
  call clap#preview#file(s:into_filename(g:clap.display.getcurline()))
endfunction

let s:files.sink = function('clap#provider#files#sink_impl')
let s:files['sink*'] = function('clap#provider#files#sink_star_impl')
let s:files.on_move = function('clap#provider#files#on_move_impl')
let s:files.enable_rooter = v:true
let s:files.support_open_action = v:true
let s:files.syntax = 'clap_files'

let g:clap#provider#files# = s:files

let &cpoptions = s:save_cpo
unlet s:save_cpo
