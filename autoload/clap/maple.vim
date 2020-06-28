" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Dispatch the job via maple.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:bin_suffix = has('win32') ? '.exe' : ''

let s:maple_bin_localbuilt = fnamemodify(g:clap#autoload_dir, ':h').'/target/release/maple'.s:bin_suffix
let s:maple_bin_prebuilt = fnamemodify(g:clap#autoload_dir, ':h').'/bin/maple'.s:bin_suffix

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

if s:maple_bin isnot v:null
  function! clap#maple#clean_up() abort
    call clap#job#regular#maple#stop()
    call clap#client#notify('exit', {})
  endfunction
else
  function! clap#maple#clean_up() abort
    call clap#job#regular#maple#stop()
  endfunction
endif

function! clap#maple#binary() abort
  return s:maple_bin
endfunction

function! clap#maple#is_available() abort
  return s:maple_bin isnot v:null
endfunction

let s:can_enable_icon = ['files', 'git_files']

function! clap#maple#forerunner_exec_command(cmd) abort
  " No global --number option.
  if g:clap_enable_icon
        \ && index(s:can_enable_icon, g:clap.provider.id) > -1
    let global_opt = ['--icon-painter=File']
  else
    let global_opt = []
  endif

  if has_key(g:clap.context, 'no-cache')
    call add(global_opt, '--no-cache')
  endif

  let subcommand = ['exec', a:cmd, '--cmd-dir', clap#rooter#working_dir(), '--output-threshold', clap#filter#capacity()]

  return [s:maple_bin] + global_opt + subcommand
endfunction

" Returns the filtered results after the input stream is complete.
function! clap#maple#sync_filter_command(query) abort
  let global_opt = ['--number', g:clap.display.preload_capacity, '--winwidth', winwidth(g:clap.display.winid)]

  if g:clap.provider.id ==# 'files' && g:clap_enable_icon
    call add(global_opt, '--icon-painter=File')
  endif

  return [s:maple_bin] + global_opt + ['filter', a:query, '--sync']
endfunction

function! clap#maple#tags_forerunner_command() abort
  let global_opt = has_key(g:clap.context, 'no-cache') ? ['--no-cache'] : []

  if g:clap_enable_icon
    call add(global_opt, '--icon-painter=ProjTags')
  endif

  return [s:maple_bin] + global_opt + ['tags', '', clap#rooter#working_dir(), '--forerunner']
endfunction

function! clap#maple#ripgrep_forerunner_command() abort
  " TODO: add max_output
  let global_opt = g:clap_enable_icon ? ['--icon-painter=Grep'] : []

  if has_key(g:clap.context, 'no-cache')
    call add(global_opt, '--no-cache')
  endif

  return [s:maple_bin] + global_opt + ['ripgrep-forerunner', '--cmd-dir', clap#rooter#working_dir(), '--output-threshold', clap#filter#capacity()]
endfunction

function! clap#maple#blines_command() abort
  let blines_subcmd = ['--number', g:clap.display.preload_capacity, '--winwidth', winwidth(g:clap.display.winid), 'blines', g:clap.input.get(), expand('#'.g:clap.start.bufnr.':p')]
  return [s:maple_bin] + blines_subcmd
endfunction

function! clap#maple#run_exec(cmd) abort
  let global_opt = ['--number', g:clap.display.preload_capacity]
  if g:clap.provider.id ==# 'files' && g:clap_enable_icon
    call add(global_opt, '--icon-painter=File')
  endif
  let subcommand = ['exec', a:cmd, '--cmd-dir', clap#rooter#working_dir()]
  call clap#job#regular#maple#start([s:maple_bin] + global_opt + subcommand)
endfunction

function! clap#maple#run_sync_grep(cmd, query, enable_icon, glob) abort
  let global_opt = ['--number', g:clap.display.preload_capacity]

  if a:enable_icon
    call add(global_opt, '--icon-painter=Grep')
  endif

  let subcommand = ['grep', a:query, '--sync', '--grep-cmd', a:cmd, '--cmd-dir', clap#rooter#working_dir()]

  if a:glob isnot v:null
    let subcommand += ['--glob', a:glob]
  endif

  call clap#job#regular#maple#start([s:maple_bin] + global_opt + subcommand)
endfunction

function! clap#maple#build_cmd(...) abort
  return [s:maple_bin] + a:000
endfunction

function! clap#maple#build_cmd_list(cmd_list) abort
  return insert(a:cmd_list, s:maple_bin)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
