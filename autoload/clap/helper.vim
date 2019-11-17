" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Helper for the cmdline completion and building extension.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:path_sep = has('win32') ? '\' : '/'

function! s:relativize(ArgLead, abs_dirs) abort
  if a:ArgLead =~# '^\~'
    return map(a:abs_dirs, 'fnamemodify(v:val, ":~")')
  elseif a:ArgLead =~# '^\.'
    if empty(a:abs_dirs)
      return []
    endif
    if fnamemodify(a:abs_dirs[0], ':.') =~# '^\.'
      return map(a:abs_dirs, 'fnamemodify(v:val, ":.")')
    else
      return map(a:abs_dirs, '".".s:path_sep.fnamemodify(v:val, ":.")')
    endif
  else
    return a:abs_dirs
  endif
endfunction

function! clap#helper#complete(ArgLead, CmdLine, P) abort
  if a:CmdLine =~# '^Clap \(files\|grep\)'
    if a:ArgLead =~# '\(/\|\\\)$' || isdirectory(expand(a:ArgLead))
      let parent_dir = fnamemodify(resolve(expand(a:ArgLead)), ':p')
      if isdirectory(parent_dir)
        let abs_dirs = filter(globpath(parent_dir, '*', 0, 1), 'isdirectory(v:val)')
        return s:relativize(a:ArgLead, abs_dirs)
      endif
    else
      let parent_dir = fnamemodify(resolve(expand(a:ArgLead)), ':h')
      if isdirectory(parent_dir)
        let abs_dirs = filter(globpath(parent_dir, '*', 0, 1), 'isdirectory(v:val) && v:val =~# "^".expand(a:ArgLead)')
        return s:relativize(a:ArgLead, abs_dirs)
      endif
    endif
  endif
  let registered = exists('g:clap') ? keys(g:clap.registrar) : []
  return filter(uniq(sort(g:clap#builtin_providers + keys(g:clap#provider_alias) + registered)), 'v:val =~# "^".a:ArgLead')
endfunction

function! clap#helper#echo_info(msg) abort
  echohl Function
  echom 'vim-clap: '.a:msg
  echohl NONE
endfunction

function! clap#helper#echo_error(msg) abort
  echohl ErrorMsg
  echom 'vim-clap: '.a:msg
  echohl NONE
endfunction

function! clap#helper#build_maple() abort
  if executable('cargo')
    let cmd = 'cargo build --release'
    10new belowright bottom
    setlocal buftype=nofile winfixheight norelativenumber nonumber bufhidden=wipe

    function! s:OnExit(status) closure
      if a:status == 0
        execute 'silent! bd! '.bufnr
        call clap#helper#echo_info('build maple successfully')
      endif
    endfunction

    if has('nvim')
      call termopen(cmd, {
            \ 'cwd': fnamemodify(g:clap#autoload_dir, ':h'),
            \ 'on_exit': {job, status -> s:OnExit(status)},
            \})
    else
      call term_start(cmd, {
            \ 'curwin': 1,
            \ 'cwd': fnamemodify(g:clap#autoload_dir, ':h'),
            \ 'exit_cb': {job, status -> s:OnExit(status)},
            \})
    endif

    let bufnr = bufnr('')

    wincmd p
  else
    call clap#helper#echo_error('Can not build maple in that cargo is not found.')
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
