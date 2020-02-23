set nocompatible

let s:cur_dir = fnamemodify(resolve(expand('<sfile>:p')), ':h:h:h:h')
execute 'set runtimepath^='.s:cur_dir

syntax on
filetype plugin indent on

source test_fuzzy_filter.vim
