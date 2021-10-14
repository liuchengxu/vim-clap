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
let s:options = filter(['fd', 'rg', 'git', 'find'], 'executable(v:val)')

if empty(s:options)
  let s:default_finder = v:null
  let s:default_source = ['No usable tools found for the files provider']
else
  let s:default_finder = s:options[0]
  let s:default_source = join([s:default_finder, s:default_opts[s:default_finder]], ' ')
endif

function! s:files.source() abort
  call clap#rooter#try_set_cwd()

  if has_key(g:clap.context, 'name-only')
    let g:__clap_match_type_enum = 'FileName'
  endif

  if has_key(g:clap.context, 'finder')
    let finder = g:clap.context.finder
    return finder.' '.join(g:clap.provider.args, ' ')
  elseif g:clap.provider.args == ['--hidden']
    if s:default_finder ==# 'fd' || s:default_finder ==# 'rg'
      return join([s:default_finder, s:default_opts[s:default_finder], '--hidden'], ' ')
    endif
  endif
  return s:default_source
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
  call clap#sink#edit_with_open_action(fpath)
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

function! s:files.on_exit() abort
  if exists('g:__clap_match_type_enum')
    unlet g:__clap_match_type_enum
  endif
endfunction

if g:__clap_development
  function! s:files.on_typed() abort
    call clap#client#call('on_typed', v:null, {'query': g:clap.input.get()})
  endfunction
endif

let s:files.sink = function('clap#provider#files#sink_impl')
let s:files['sink*'] = function('clap#provider#files#sink_star_impl')
let s:files.on_move = function('clap#provider#files#on_move_impl')
let s:files.on_move_async = function('clap#impl#on_move#async')
let s:files.enable_rooter = v:true
let s:files.support_open_action = v:true
let s:files.syntax = 'clap_files'

let g:clap#provider#files# = s:files

let &cpoptions = s:save_cpo
unlet s:save_cpo
