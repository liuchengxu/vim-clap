" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Utils for building the maple command in CLI.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:maple_bin = clap#maple#binary()

let s:can_enable_icon = ['files', 'git_files']

function! clap#maple#command#grep_sync(cmd, query, enable_icon, glob) abort
  let global_opt = ['--number', g:clap.display.preload_capacity]

  if a:enable_icon
    call add(global_opt, '--icon-painter=Grep')
  endif

  let subcommand = [
        \ 'grep', a:query,
        \ '--sync',
        \ '--grep-cmd', a:cmd,
        \ '--cmd-dir', clap#rooter#working_dir(),
        \ ]

  if a:glob isnot v:null
    let subcommand += ['--glob', a:glob]
  endif

  call clap#job#regular#maple#start([s:maple_bin] + global_opt + subcommand)
endfunction

function! clap#maple#command#ripgrep_forerunner() abort
  " TODO: add max_output
  let global_opt = g:clap_enable_icon ? ['--icon-painter=Grep'] : []

  if has_key(g:clap.context, 'no-cache')
    call add(global_opt, '--no-cache')
  endif

  let subcommand = [
        \ 'ripgrep-forerunner',
        \ '--cmd-dir', clap#rooter#working_dir(),
        \ '--output-threshold', clap#filter#capacity(),
        \ ]

  return [s:maple_bin] + global_opt + subcommand
endfunction

function! clap#maple#command#exec_forerunner(cmd) abort
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

  let subcommand = [
        \ 'exec', a:cmd,
        \ '--cmd-dir', clap#rooter#working_dir(),
        \ '--output-threshold', clap#filter#capacity(),
        \ ]

  return [s:maple_bin] + global_opt + subcommand
endfunction

" Returns the filtered results after the input stream is complete.
function! clap#maple#command#filter_sync(query) abort
  let global_opt = ['--number', g:clap.display.preload_capacity, '--winwidth', winwidth(g:clap.display.winid)]

  if g:clap.provider.id ==# 'files'
    let tmp = tempname()
    call writefile(clap#util#get_mru_list(), tmp)
    call add(global_opt, printf('--recent-files=%s', tmp))

    call add(global_opt, printf('--bonus=%s', clap#filter#get_bonus_type()))
    if g:clap_enable_icon
      call add(global_opt, '--icon-painter=File')
    endif
  endif

  return [s:maple_bin] + global_opt + ['filter', a:query, '--sync']
endfunction

function! clap#maple#command#tags(is_forerunner) abort
  let global_opt = has_key(g:clap.context, 'no-cache') ? ['--no-cache'] : []

  if g:clap_enable_icon
    call add(global_opt, '--icon-painter=ProjTags')
  endif

  if a:is_forerunner
    let subcommand = ['tags', '', clap#rooter#working_dir(), '--forerunner']
  else
    let subcommand = ['tags', '', clap#rooter#working_dir()]
  endif

  return [s:maple_bin] + global_opt + subcommand
endfunction

function! clap#maple#command#blines() abort
  let full_command = [
        \ '--number', g:clap.display.preload_capacity,
        \ '--winwidth', winwidth(g:clap.display.winid),
        \ 'blines', g:clap.input.get(),
        \ expand('#'.g:clap.start.bufnr.':p')
        \ ]
  return [s:maple_bin] + full_command
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
