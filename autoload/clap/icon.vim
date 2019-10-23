" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Icon decorator, derived from vim-devicons.

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

let g:clap#icon#default = ''

let g:clap#icon#extensions = {
      \ 'styl'     : '',
      \ 'sass'     : '',
      \ 'scss'     : '',
      \ 'htm'      : '',
      \ 'html'     : '',
      \ 'slim'     : '',
      \ 'ejs'      : '',
      \ 'css'      : '',
      \ 'less'     : '',
      \ 'md'       : '',
      \ 'markdown' : '',
      \ 'rmd'      : '',
      \ 'json'     : '',
      \ 'js'       : '',
      \ 'jsx'      : '',
      \ 'rb'       : '',
      \ 'php'      : '',
      \ 'py'       : '',
      \ 'pyc'      : '',
      \ 'pyo'      : '',
      \ 'pyd'      : '',
      \ 'coffee'   : '',
      \ 'mustache' : '',
      \ 'hbs'      : '',
      \ 'conf'     : '',
      \ 'ini'      : '',
      \ 'yml'      : '',
      \ 'yaml'     : '',
      \ 'toml'     : '',
      \ 'bat'      : '',
      \ 'jpg'      : '',
      \ 'jpeg'     : '',
      \ 'bmp'      : '',
      \ 'png'      : '',
      \ 'gif'      : '',
      \ 'ico'      : '',
      \ 'twig'     : '',
      \ 'cpp'      : '',
      \ 'c++'      : '',
      \ 'cxx'      : '',
      \ 'cc'       : '',
      \ 'cp'       : '',
      \ 'c'        : '',
      \ 'h'        : '',
      \ 'hpp'      : '',
      \ 'hxx'      : '',
      \ 'hs'       : '',
      \ 'lhs'      : '',
      \ 'lua'      : '',
      \ 'java'     : '',
      \ 'sh'       : '',
      \ 'fish'     : '',
      \ 'bash'     : '',
      \ 'zsh'      : '',
      \ 'ksh'      : '',
      \ 'csh'      : '',
      \ 'awk'      : '',
      \ 'ps1'      : '',
      \ 'ml'       : 'λ',
      \ 'mli'      : 'λ',
      \ 'diff'     : '',
      \ 'db'       : '',
      \ 'sql'      : '',
      \ 'dump'     : '',
      \ 'clj'      : '',
      \ 'cljc'     : '',
      \ 'cljs'     : '',
      \ 'edn'      : '',
      \ 'scala'    : '',
      \ 'go'       : '',
      \ 'dart'     : '',
      \ 'xul'      : '',
      \ 'sln'      : '',
      \ 'suo'      : '',
      \ 'pl'       : '',
      \ 'pm'       : '',
      \ 't'        : '',
      \ 'rss'      : '',
      \ 'f#'       : '',
      \ 'fsscript' : '',
      \ 'fsx'      : '',
      \ 'fs'       : '',
      \ 'fsi'      : '',
      \ 'rs'       : '',
      \ 'rlib'     : '',
      \ 'd'        : '',
      \ 'erl'      : '',
      \ 'hrl'      : '',
      \ 'ex'       : '',
      \ 'exs'      : '',
      \ 'eex'      : '',
      \ 'vim'      : '',
      \ 'ai'       : '',
      \ 'psd'      : '',
      \ 'psb'      : '',
      \ 'ts'       : '',
      \ 'tsx'      : '',
      \ 'jl'       : '',
      \ 'pp'       : '',
      \ 'vue'      : '﵂',
      \ 'lock'     : 'ﲁ',
      \ 'swift'    : '',
      \ 'xcplayground' : ''
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
  let extensions = values(g:clap#icon#extensions)
  let exact_matches = values(g:clap#icon#exact_matches)
  let pattern_matches = values(g:clap#icon#pattern_matches)
  let all = []
  call extend(all, extensions + exact_matches + pattern_matches)
  call add(all, g:clap#icon#default)
  return uniq(all)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
