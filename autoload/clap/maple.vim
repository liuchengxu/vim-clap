" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Dispatch the job via maple.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:bin_suffix = has('win32') ? '.exe' : ''

let s:maple_bin_localbuilt = fnamemodify(g:clap#autoload_dir, ':h').'/target/release/maple'.s:bin_suffix
let s:maple_bin_prebuilt = fnamemodify(g:clap#autoload_dir, ':h').'/bin/maple'.s:bin_suffix

function! s:find_executable() abort
  " Check the local built.
  if executable(s:maple_bin_localbuilt)
    let s:maple_bin = s:maple_bin_localbuilt
  " Check the prebuilt binary.
  elseif executable(s:maple_bin_prebuilt)
    let s:maple_bin = s:maple_bin_prebuilt
  elseif executable('maple')
    let s:maple_bin = 'maple'
  else
    let s:maple_bin = v:null
  endif
endfunction

call s:find_executable()

function! clap#maple#reload() abort
  call s:find_executable()
endfunction

function! clap#maple#clean_up() abort
  if s:maple_bin isnot v:null
    call clap#client#notify_provider('exit_provider')
  endif
endfunction

function! clap#maple#binary() abort
  return s:maple_bin
endfunction

function! clap#maple#is_available() abort
  return s:maple_bin isnot v:null
endfunction

function! clap#maple#build_cmd(...) abort
  return [s:maple_bin] + a:000
endfunction

function! clap#maple#build_cmd_list(cmd_list) abort
  return insert(a:cmd_list, s:maple_bin)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
