" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Icon decorator, derived from vim-devicons.

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

let g:clap#icon#default = ''

let s:plugin_root_dir = fnamemodify(g:clap#autoload_dir, ':h')

function! s:deserialize_json(json_file) abort
  return json_decode(join(readfile(a:json_file), ''))
endfunction

" Convert an icon char to hex: printf("%x", char2nr('C'))
if !exists('g:clap#icon#extensions')
  let g:clap#icon#extensions = s:deserialize_json(s:plugin_root_dir.'/crates/icon/extension_map.json')
endif

if !exists('g:clap#icon#exact_matches')
  let g:clap#icon#exact_matches = s:deserialize_json(s:plugin_root_dir.'/crates/icon/exactmatch_map.json')
endif

let g:clap#icon#pattern_matches = {
      \ '.*jquery.*\.js$'       : '',
      \ '.*angular.*\.js$'      : '',
      \ '.*backbone.*\.js$'     : '',
      \ '.*require.*\.js$'      : '',
      \ '.*materialize.*\.js$'  : '',
      \ '.*materialize.*\.css$' : '',
      \ '.*mootools.*\.js$'     : '',
      \ '.*vimrc.*'             : '',
      \ 'Vagrantfile$'          : ''
      \}

function! clap#icon#get(pattern) abort
  let ext = fnamemodify(a:pattern, ':e')
  if empty(ext)
    return get(g:clap#icon#exact_matches, a:pattern, g:clap#icon#default)
  else
    return get(g:clap#icon#extensions, ext, g:clap#icon#default)
  endif
endfunction

function! s:icon_for(k) abort
  return get(g:clap#icon#extensions, a:k, g:clap#icon#default)
endfunction

function! clap#icon#for(bufname) abort
  let ext = fnamemodify(expand(a:bufname), ':e')
  if empty(ext)
    let ft = getbufvar(a:bufname, '&ft')
    return empty(ft) ? g:clap#icon#default : s:icon_for(ft)
  else
    return s:icon_for(ext)
  endif
endfunction

function! clap#icon#get_all() abort
  if !exists('s:icon_set')
    let extensions = values(g:clap#icon#extensions)
    let exact_matches = values(g:clap#icon#exact_matches)
    let pattern_matches = values(g:clap#icon#pattern_matches)
    let s:icon_set = [' ']
    call extend(s:icon_set, extensions + exact_matches + pattern_matches)
    call add(s:icon_set, g:clap#icon#default)
    let s:icon_set = uniq(s:icon_set)
  endif
  return s:icon_set
endfunction

function! s:get_color(group, attr) abort
  return synIDattr(synIDtrans(hlID(a:group)), a:attr)
endfunction

function! s:get_attrs(group) abort
  let fg = s:get_color(a:group, 'fg')
  if empty(fg)
    let fg = s:normal_fg
  endif
  " guibg=NONE ctermbg=NONE is neccessary otherwise the bg could be unexpected.
  return printf('%sbg=%s %sfg=%s guibg=NONE ctermbg=NONE', s:gui_or_cterm, s:normal_bg, s:gui_or_cterm, fg)
endfunction

function! clap#icon#def_color_components() abort
  " TODO: more robust gui_running detection for neovim, ref #378
  let s:use_gui = exists('g:neovide') || (has('termguicolors') && &termguicolors) || (!has('nvim') && has('gui_running'))
  let s:gui_or_cterm = s:use_gui ? 'gui' : 'cterm'

  let s:normal_fg = s:get_color('Normal', 'fg')
  if empty(s:normal_fg)
    let s:normal_fg = s:gui_or_cterm ==# 'gui' ? '#b2b2b2' : 249
  endif

  let s:normal_bg = s:get_color('Normal', 'bg')
  if empty(s:normal_bg)
    let s:normal_bg = s:gui_or_cterm ==# 'gui' ? '#292b2e' : 235
  endif
endfunction

let s:linked_groups = [
      \ 'ModeMsg',
      \ 'Type',
      \ 'Number',
      \ 'Float',
      \ 'Question',
      \ 'Title',
      \ 'Identifier',
      \ 'Repeat',
      \ 'Keyword',
      \ 'Constant',
      \ 'String',
      \ 'Character',
      \ 'Statement',
      \ 'WildMenu',
      \ 'Folded',
      \ 'FoldColumn',
      \ 'DiffAdd',
      \ 'DiffChange',
      \ 'DiffText',
      \ 'Function',
      \ 'Define',
      \ ]

let s:linked_groups_len = len(s:linked_groups)

call clap#icon#def_color_components()

function! s:generic_hi_icons(head_only) abort
  let pat_prefix = a:head_only ? '/^\s*' : '/'

  let lk_idx = 0
  let groups = []
  let icons = clap#icon#get_all()
  for idx in range(len(icons))
    let group = 'ClapIcon'.idx
    call add(groups, group)
    execute 'syntax match' group pat_prefix.icons[idx].'/' 'contained'
    execute 'hi!' group s:get_attrs(s:linked_groups[lk_idx])
    let lk_idx += 1
    let lk_idx = lk_idx % s:linked_groups_len
  endfor

  return groups
endfunction

function! clap#icon#add_hl_groups() abort
  return s:generic_hi_icons(v:false)
endfunction

function! clap#icon#add_head_hl_groups() abort
  return s:generic_hi_icons(v:true)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
