" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Utils for building the maple command in CLI.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:maple_bin = clap#maple#binary()

let s:can_enable_icon = ['files', 'git_files']

function! clap#maple#command#start_grep_sync(cmd, query, enable_icon, glob) abort
  let global_opts = ['--number', g:clap.display.preload_capacity, '--winwidth', winwidth(g:clap.display.winid)]

  if a:enable_icon
    call add(global_opts, '--icon=Grep')
  endif

  let subcommand = [
        \ 'grep', a:query,
        \ '--grep-cmd', a:cmd,
        \ '--cmd-dir', clap#rooter#working_dir(),
        \ '--sync',
        \ ]

  if a:glob isnot v:null
    let subcommand += ['--glob', a:glob]
  endif

  call clap#job#regular#maple#start([s:maple_bin] + global_opts + subcommand)
endfunction

function! clap#maple#command#ripgrep_forerunner() abort
  " TODO: add max_output
  let global_opts = g:clap_enable_icon ? ['--icon=Grep'] : []

  if has_key(g:clap.context, 'no-cache')
    call add(global_opts, '--no-cache')
  endif

  let subcommand = [
        \ 'ripgrep-forerunner',
        \ '--cmd-dir', clap#rooter#working_dir(),
        \ '--output-threshold', clap#filter#capacity(),
        \ ]

  return [s:maple_bin] + global_opts + subcommand
endfunction

function! s:inject_icon_opt(opts) abort
  let global_opts = a:opts
  if g:clap_enable_icon
    if index(['files', 'git_files'], g:clap.provider.id) > -1
      call add(global_opts, '--icon=File')
    elseif 'proj_tags' ==# g:clap.provider.id
      call add(global_opts, '--icon=ProjTags')
    endif
  endif
  return global_opts
endfunction

function! clap#maple#command#exec_forerunner(cmd) abort
  " No global --number option.
  let global_opts = s:inject_icon_opt([])

  if has_key(g:clap.context, 'no-cache')
    call add(global_opts, '--no-cache')
  endif

  let subcommand = [
        \ 'exec', a:cmd,
        \ '--cmd-dir', clap#rooter#working_dir(),
        \ '--output-threshold', clap#filter#capacity(),
        \ ]

  return [s:maple_bin] + global_opts + subcommand
endfunction

" Returns the filtered results after the input stream is complete.
function! clap#maple#command#filter_sync(query) abort
  let global_opts = ['--number', g:clap.display.preload_capacity, '--winwidth', winwidth(g:clap.display.winid)]

  if g:clap.provider.id ==# 'files'
    let tmp = tempname()
    call writefile(clap#util#recent_files(), tmp)
    call add(global_opts, printf('--recent-files=%s', tmp))

    call add(global_opts, printf('--bonus=%s', clap#filter#get_bonus_type()))
    if g:clap_enable_icon
      call add(global_opts, '--icon=File')
    endif
  endif

  return [s:maple_bin] + global_opts + ['filter', a:query, '--sync']
endfunction

function! clap#maple#command#filter_dyn(dyn_size, tempfile) abort
  let global_opts = ['--number', a:dyn_size, '--winwidth', winwidth(g:clap.display.winid)]
  let global_opts = s:inject_icon_opt(global_opts)

  let subcommand = [
        \ 'filter', g:.clap.input.get(),
        \ '--input', a:tempfile,
        \ ]

  if g:clap.provider.id ==# 'files'
    if has_key(g:clap.context, 'name-only')
      call add(subcommand, '--match-type=FileName')
    endif
    if !exists('g:__clap_recent_files_dyn_tmp')
      let g:__clap_recent_files_dyn_tmp = tempname()
      call writefile(clap#util#recent_files(), g:__clap_recent_files_dyn_tmp)
    endif
    call add(subcommand, printf('--recent-files=%s', g:__clap_recent_files_dyn_tmp))
  else
    if g:clap.provider.id ==# 'proj_tags'
      call add(subcommand, '--match-type=TagName')
    endif
  endif

  return [s:maple_bin] + global_opts + subcommand
endfunction

function! clap#maple#command#tags(is_forerunner) abort
  let global_opts = has_key(g:clap.context, 'no-cache') ? ['--no-cache'] : []
  let global_opts = s:inject_icon_opt(global_opts)

  let subcommand = ['ctags', 'recursive-tags']

  if a:is_forerunner
    call add(subcommand, '--forerunner')
  endif

  let subcommand = subcommand + ['--dir', clap#rooter#working_dir()]

  return [s:maple_bin] + global_opts + subcommand
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
