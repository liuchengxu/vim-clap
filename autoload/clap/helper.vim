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

function! clap#helper#echo_warn(msg) abort
  echohl WarningMsg
  echom 'vim-clap: '.a:msg
  echohl NONE
endfunction

function! s:run_term(cmd, cwd, success_info) abort
  10new belowright bottom
  setlocal buftype=nofile winfixheight norelativenumber nonumber bufhidden=wipe

  function! s:OnExit(status) closure abort
    if a:status == 0
      execute 'silent! bd! '.bufnr
      call clap#helper#echo_info(a:success_info)
    endif
  endfunction

  if has('nvim')
    call termopen(a:cmd, {
          \ 'cwd': a:cwd,
          \ 'on_exit': {job, status -> s:OnExit(status)},
          \})
  else
    call term_start(a:cmd, {
          \ 'curwin': 1,
          \ 'cwd': a:cwd,
          \ 'exit_cb': {job, status -> s:OnExit(status)},
          \})
  endif

  let bufnr = bufnr('')

  normal! G

  noautocmd wincmd p
endfunction

if has('win32')
  let s:from = '.\fuzzymatch-rs\target\release\libfuzzymatch_rs.dll'
  let s:to = 'libfuzzymatch_rs.pyd'
  let s:rust_ext_cmd = printf('cargo +nightly build --release && copy %s %s', s:from, s:to)
  let s:rust_ext_cwd = fnamemodify(g:clap#autoload_dir, ':h').'\pythonx\clap'
else
  let s:rust_ext_cmd = 'make build'
  let s:rust_ext_cwd = fnamemodify(g:clap#autoload_dir, ':h').'/pythonx/clap'
endif

function! s:has_rust_nightly(show_warning) abort
  call system('cargo +nightly --help')
  if v:shell_error
    if a:show_warning
      call clap#helper#echo_warn('Rust nightly is required, try running `rustup toolchain install nightly` in the command line and then rerun this function.')
    else
      call clap#helper#echo_info('Rust nightly is required, skip building the Python dynamic module.')
    endif
    return v:false
  endif
  return v:true
endfunction

function! clap#helper#build_python_dynamic_module() abort
  if !has('python3')
    call clap#helper#echo_info('+python3 is required, skip building the Python dynamic module.')
    return
  endif

  if executable('cargo')
    if !s:has_rust_nightly(v:true)
      return
    endif
    call s:run_term(s:rust_ext_cmd, s:rust_ext_cwd, 'build Rust extension successfully')
  else
    call clap#helper#echo_error('Can not build Rust extension in that cargo is not found.')
  endif
endfunction

function! clap#helper#build_maple() abort
  if executable('cargo')
    let cmd = 'cargo build --release'
    let cwd = fnamemodify(g:clap#autoload_dir, ':h')
    call s:run_term(cmd, cwd, 'build maple successfully')
  else
    call clap#helper#echo_error('Can not build maple in that cargo is not found.')
  endif
endfunction

function! clap#helper#build_all(...) abort
  if executable('cargo')
    " If Rust nightly is unavailable, build the maple only.
    if s:has_rust_nightly(v:false)
      let cwd = fnamemodify(g:clap#autoload_dir, ':h')
      if has('win32')
        let cmd = printf('cargo build --release && cd /d %s && %s', s:rust_ext_cwd, s:rust_ext_cmd)
      else
        let cmd = 'make'
      endif
      call s:run_term(cmd, cwd, 'build maple and Rust extension successfully')
    else
      call clap#helper#build_maple()
    endif
  else
    call clap#helper#echo_warn('cargo not found, skipped building maple and the Rust extension.')
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
