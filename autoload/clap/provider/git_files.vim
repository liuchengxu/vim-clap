" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the files managed by git.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:git_files = {}

if executable('git')
  let s:git_files.source = 'git ls-files '.(has('win32') ? '' : ' | uniq')
else
  let s:git_files.source = ['git executable not found']
endif

let s:git_files.sink = function('clap#provider#files#sink_impl')
let s:git_files['sink*'] = function('clap#provider#files#sink_star_impl')
let s:git_files.on_move = function('clap#provider#files#on_move_impl')
let s:git_files.syntax = 'clap_files'
let s:git_files.enable_rooter = v:true
let s:git_files.support_open_action = v:true

let g:clap#provider#git_files# = s:git_files

let &cpoptions = s:save_cpo
unlet s:save_cpo
