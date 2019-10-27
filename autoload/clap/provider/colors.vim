" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the colorschemes.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:colors = {}

" Derived from fzf.vim
function! s:colors.source() abort
  let colors = split(globpath(&runtimepath, 'colors/*.vim'), "\n")
  if has('packages')
    let colors += split(globpath(&packpath, 'pack/*/opt/*/colors/*.vim'), "\n")
  endif
  return map(colors, "substitute(fnamemodify(v:val, ':t'), '\\..\\{-}$', '', '')")
endfunction

function! s:colors.on_enter() abort
  redir => s:old_color
  silent colorscheme
  redir END
  let s:old_bg = &background
endfunction

" Preview the colorscheme on move
function! s:colors.on_move() abort
  " This is neccessary
  call g:clap.start.goto_win()
  execute 'color' g:clap.display.getcurline()
  do Syntax
  call g:clap.input.goto_win()
endfunction

function! s:colors.sink(selected) abort
  execute 'color' a:selected
  " Reload syntax
  " https://stackoverflow.com/questions/8674387/vim-how-to-reload-syntax-highlighting
  do Syntax
  let s:should_restore_color = v:false
endfunction

function! s:colors.on_exit() abort
  if get(s:, 'should_restore_color', v:true)
    call g:clap.start.goto_win()
    execute 'color' trim(s:old_color)
    let &background = s:old_bg
    let s:should_restore_color = v:true
  endif
endfunction

let g:clap#provider#colors# = s:colors

let &cpoptions = s:save_cpo
unlet s:save_cpo
