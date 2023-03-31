" Author: romgrk <romgrk.cc@gmail.com>
" Description: Project-wide tags

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:provider = {}

function! s:provider.on_typed() abort
  call clap#client#notify('on_typed')
endfunction

function! s:provider.init() abort
  call clap#client#notify_on_init()
endfunction

function! s:provider.sink(selected) abort
  call s:jump_to(s:extract(a:selected))
  try
    silent! call vista#util#Blink(2, 200)
  catch '*' | endtry
endfunction

function! s:provider.on_move() abort
  let [path, address] = s:extract(g:clap.display.getcurline())
  call clap#preview#file_at(path, address)
endfunction

let s:provider.on_move_async = function('clap#impl#on_move#async')
let s:provider.enable_rooter = v:true
let s:provider.support_open_action = v:true
let s:provider.syntax = 'clap_tagfiles'

let g:clap#provider#tagfiles# = s:provider

" Helpers

function! s:extract(tag_row) abort
  let parts = split(a:tag_row, '::::')
  " let line = parts[0]
  let file    = parts[1]
  let address = parts[2]
  if address[0] == '/'
    " Format: `/^function example()$/`
    " inside the `/^` and `$/` is like nomagic, but some ctags program
    " put the ^ and $ anyway.
    let address = address[1:-2]
    if address[0] == '^'
      let address = '\v^\V' . address[1:]
    else
      let address = '\V' . address
    end
    if address[-1:] == '$'
      let address = address[:-2] . '\v$'
    end
  else
    let address = str2nr(matchstr(address, '\v\d+'))
  end
  return [file, address]
endfunction

function! s:jump_to(position)
  let [file, address] = a:position

  execute 'edit' file

  if type(address) == v:t_number
    let lnum = address
    execute 'normal! ' lnum 'gg'
  else
    let g:cp = address
    let lnum = search(address)
  end

  execute 'normal! ^zvzz'
endfunc

let &cpoptions = s:save_cpo
unlet s:save_cpo
