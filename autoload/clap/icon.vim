" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Icon decorator, derived from vim-devicons.

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

let g:clap#icon#default = ''

let g:clap#icon#extensions = {
      \ 'styl'         : '',
      \ 'sass'         : '',
      \ 'scss'         : '',
      \ 'htm'          : '',
      \ 'html'         : '',
      \ 'slim'         : '',
      \ 'ejs'          : '',
      \ 'css'          : '',
      \ 'less'         : '',
      \ 'md'           : '',
      \ 'markdown'     : '',
      \ 'rmd'          : '',
      \ 'json'         : '',
      \ 'js'           : '',
      \ 'jsx'          : '',
      \ 'rb'           : '',
      \ 'php'          : '',
      \ 'py'           : '',
      \ 'pyc'          : '',
      \ 'pyo'          : '',
      \ 'pyd'          : '',
      \ 'coffee'       : '',
      \ 'mustache'     : '',
      \ 'hbs'          : '',
      \ 'conf'         : '',
      \ 'ini'          : '',
      \ 'yml'          : '',
      \ 'yaml'         : '',
      \ 'toml'         : '',
      \ 'bat'          : '',
      \ 'jpg'          : '',
      \ 'jpeg'         : '',
      \ 'bmp'          : '',
      \ 'png'          : '',
      \ 'gif'          : '',
      \ 'ico'          : '',
      \ 'twig'         : '',
      \ 'cpp'          : '',
      \ 'c++'          : '',
      \ 'cxx'          : '',
      \ 'cc'           : '',
      \ 'cp'           : '',
      \ 'c'            : '',
      \ 'h'            : '',
      \ 'hpp'          : '',
      \ 'hxx'          : '',
      \ 'hs'           : '',
      \ 'lhs'          : '',
      \ 'lua'          : '',
      \ 'java'         : '',
      \ 'sh'           : '',
      \ 'fish'         : '',
      \ 'bash'         : '',
      \ 'zsh'          : '',
      \ 'ksh'          : '',
      \ 'csh'          : '',
      \ 'awk'          : '',
      \ 'ps1'          : '',
      \ 'ml'           : 'λ',
      \ 'mli'          : 'λ',
      \ 'diff'         : '',
      \ 'db'           : '',
      \ 'sql'          : '',
      \ 'dump'         : '',
      \ 'clj'          : '',
      \ 'cljc'         : '',
      \ 'cljs'         : '',
      \ 'edn'          : '',
      \ 'scala'        : '',
      \ 'go'           : '',
      \ 'dart'         : '',
      \ 'xul'          : '',
      \ 'sln'          : '',
      \ 'suo'          : '',
      \ 'pl'           : '',
      \ 'pm'           : '',
      \ 't'            : '',
      \ 'rss'          : '',
      \ 'f#'           : '',
      \ 'fsscript'     : '',
      \ 'fsx'          : '',
      \ 'fs'           : '',
      \ 'fsi'          : '',
      \ 'rs'           : '',
      \ 'rlib'         : '',
      \ 'rmeta'        : '',
      \ 'd'            : '',
      \ 'erl'          : '',
      \ 'hrl'          : '',
      \ 'ex'           : '',
      \ 'exs'          : '',
      \ 'eex'          : '',
      \ 'vim'          : '',
      \ 'vimrc'        : '',
      \ 'ai'           : '',
      \ 'psd'          : '',
      \ 'psb'          : '',
      \ 'ts'           : '',
      \ 'tsx'          : '',
      \ 'jl'           : '',
      \ 'pp'           : '',
      \ 'vue'          : '﵂',
      \ 'swift'        : '',
      \ 'xcplayground' : '',
      \ 'lock'         : '',
      \ 'bin'          : '',
      \ 'timestamp'    : '﨟',
      \ 'txt'          : '',
      \ 'log'          : '',
      \ 'plist'        : '况',
      \ 'dylib'        : '',
      \ 'so'           : '',
      \ 'gz'           : '',
      \ 'zip'          : '',
      \}

let g:clap#icon#exact_matches = {
      \ 'exact-match-case-sensitive-1.txt' : '1',
      \ 'exact-match-case-sensitive-2'     : '2',
      \ 'gruntfile.coffee'                 : '',
      \ 'gruntfile.js'                     : '',
      \ 'gruntfile.ls'                     : '',
      \ 'gulpfile.coffee'                  : '',
      \ 'gulpfile.js'                      : '',
      \ 'gulpfile.ls'                      : '',
      \ 'dropbox'                          : '',
      \ '.ds_store'                        : '',
      \ '.gitconfig'                       : '',
      \ '.gitignore'                       : '',
      \ '.bashrc'                          : '',
      \ '.zshrc'                           : '',
      \ '.vimrc'                           : '',
      \ '.gvimrc'                          : '',
      \ '_vimrc'                           : '',
      \ '_gvimrc'                          : '',
      \ '.bashprofile'                     : '',
      \ 'favicon.ico'                      : '',
      \ 'license'                          : '',
      \ 'node_modules'                     : '',
      \ 'react.jsx'                        : '',
      \ 'procfile'                         : '',
      \ 'dockerfile'                       : '',
      \ 'docker-compose.yml'               : '',
      \}

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
  if !empty(ext)
    return get(g:clap#icon#extensions, ext, g:clap#icon#default)
  else
    return get(g:clap#icon#exact_matches, a:pattern, g:clap#icon#default)
  endif
endfunction

function! s:icon_for(k) abort
  return get(g:clap#icon#extensions, a:k, g:clap#icon#default)
endfunction

function! clap#icon#for(bufname) abort
  let ext = fnamemodify(expand(a:bufname), ':e')
  if empty(ext)
    let ft = getbufvar(a:bufname, '&ft')
    if empty(ft)
      return g:clap#icon#default
    else
      return s:icon_for(ft)
    endif
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
  let s:use_gui = has('gui_running') || (has('termguicolors') && &termguicolors)
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
