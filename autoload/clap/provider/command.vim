" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List the command.

let s:save_cpo = &cpoptions
set cpoptions&vim


let s:command_header   = v:null
let s:command_list = []

function! s:sink(selected) abort
  " :h command, note the characters in the first two columns
  let cmd = matchstr(a:selected, '\v^\s*\S+\s+\zs\S+')
  let list = split(execute('command ' . cmd), "\n")
  let command = s:parse_command(list[1])
  if command.args ==# '0'
    execute cmd
  else
    call feedkeys(':' . cmd . ' ', 'n')
  end
endfunction

function! s:source() abort
  let margin = 9
  let list = split(execute('command'), "\n")
  let s:command_header = list[0]
  let s:command_list = list[1:]
  call map(s:command_list, {key, val -> s:parse_command(val)})
  return map(copy(s:command_list), {key, val ->
        \ s:left_pad(printf('[%s] ', val.args), margin)
        \ . s:right_pad(val.name, 30)
        \ . ' ' . val.rest})
endfunction


let s:command = {}
let s:command.syntax = 'clap_command'
let s:command.source = function('s:source')
let s:command.sink = function('s:sink')

let g:clap#provider#command# = s:command

" Helpers

function! s:parse_command(input) abort
  let bang = stridx(a:input[0:3], '!') != -1
  let input = a:input[4:]
  let name = matchstr(input, '\v^\S+')
  let input = input[len(name):]
  let args = matchstr(input, '\v\s+\S+')
  let input = input[len(args):]
  let args = trim(args)
  let rest = trim(input)
  " let input = trim(input)
  " let input = input[len(args):]
  " let [sequence, address, complete] = matchlist(input, '\v *(\S+)? *(\S+) *')
  return { 'name': name, 'bang': bang, 'args': args, 'rest': rest }
endfunc

" NOTE: in case we want to do something with this eventually...
" let complete_patterns = [
"   \ 'arglist', 'augroup', 'buffer', 'behave', 'color', 'command', 'compiler', 'cscope',
"   \ 'dir', 'environment', 'event', 'expression', 'file', 'file_in_path', 'filetype',
"   \ 'function', 'help', 'highlight', 'history', 'locale', 'mapclear', 'mapping',
"   \ 'menu', 'messages', 'option', 'packadd', 'shellcmd', 'sign', 'syntax', 'syntime',
"   \ 'tag', 'tag_listfiles', 'user', 'var', 'custom,\S+', 'customlist,\S+' ]

function! s:right_pad(s, n) abort
    return a:s . repeat(' ', a:n - len(a:s))
endfunction

function! s:left_pad(s, n) abort
    return repeat(' ', a:n - len(a:s)) . a:s
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
