" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Quick installer for the extra clap tools.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:plugin_root_dir = fnamemodify(g:clap#autoload_dir, ':h')

function! s:run_term(cmd, cwd, success_info) abort
  belowright 10new
  setlocal buftype=nofile winfixheight norelativenumber nonumber bufhidden=wipe

  let bufnr = bufnr('')

  function! s:OnExit(status) closure abort
    if a:status == 0
      execute 'silent! bd! '.bufnr
      call clap#helper#echo_info(a:success_info)
    endif
    if clap#maple#is_available()
      call clap#job#daemon#start(function('clap#client#handle'))
    endif
  endfunction

  if has('nvim')
    call termopen(a:cmd, {
          \ 'cwd': a:cwd,
          \ 'on_exit': {job, status -> s:OnExit(status)},
          \ 'env': {'MAKE_CMD': s:make_cmd},
          \})
  else
    let cmd = a:cmd
    if has('win32')
      let cmd = 'cmd.exe /c '.cmd
    endif
    call term_start(cmd, {
          \ 'curwin': 1,
          \ 'cwd': a:cwd,
          \ 'exit_cb': {job, status -> s:OnExit(status)},
          \})
  endif

  normal! G

  noautocmd wincmd p
endfunction

let s:make_cmd = 'make'
if has('win32')
  let s:from = '.\fuzzymatch-rs\target\release\fuzzymatch_rs.dll'
  let s:to = 'fuzzymatch_rs.pyd'
  let s:rust_ext_cmd = printf('pushd fuzzymatch-rs && cargo build --release && popd && copy %s %s', s:from, s:to)
  let s:rust_ext_cwd = s:plugin_root_dir.'\pythonx\clap'
  let s:prebuilt_maple_binary = s:plugin_root_dir.'\bin\maple.exe'
  let s:maple_cargo_toml = s:plugin_root_dir.'\Cargo.toml'
else
  let s:rust_ext_cmd = s:make_cmd . ' build'
  let s:rust_ext_cwd = s:plugin_root_dir.'/pythonx/clap'
  let s:prebuilt_maple_binary = s:plugin_root_dir.'/bin/maple'
  let s:maple_cargo_toml = s:plugin_root_dir.'/Cargo.toml'
endif

function! clap#installer#build_python_dynamic_module() abort
  if !has('python3')
    call clap#helper#echo_info('+python3 is required, skip building the Python dynamic module.')
    return
  endif

  if executable('cargo')
    if !s:unix_sanity_check_is_ok()
      return
    endif
    call s:run_term(s:rust_ext_cmd, s:rust_ext_cwd, 'built Python dynamic module successfully')
  else
    call clap#helper#echo_error('Can not build Python dynamic module in that cargo is not found.')
  endif
endfunction

function! clap#installer#build_maple() abort
  if executable('cargo')
    let cmd = 'cargo build --release'
    call s:run_term(cmd, s:plugin_root_dir, 'built maple binary successfully')
  else
    call clap#helper#echo_error('Can not build maple binary in that cargo is not found.')
  endif
endfunction

function! clap#installer#build_all(...) abort
  if executable('cargo')
    " If Rust nightly and +python3 is unavailable, build the maple only.
    if has('python3')
      if has('win32')
        let cmd = printf('cargo build --release && cd /d %s && %s', s:rust_ext_cwd, s:rust_ext_cmd)
      else
        if !s:unix_sanity_check_is_ok()
          return
        endif
        let cmd = s:make_cmd
      endif
      call s:run_term(cmd, s:plugin_root_dir, 'built maple binary and Python dynamic module successfully')
    else
      call clap#installer#build_maple()
    endif
  else
    call clap#helper#echo_warn('cargo not found, skip building maple binary and Python dynamic module.')
  endif
endfunction

function! clap#installer#download_binary() abort
  if has('win32')
    let cmd = 'Powershell.exe -ExecutionPolicy ByPass -File "'.s:plugin_root_dir.'\install.ps1"'
  else
    if !s:unix_sanity_check_is_ok()
      return
    endif
    let cmd = './install.sh'
  endif
  call s:run_term(cmd, s:plugin_root_dir, 'download the prebuilt maple binary successfully')
endfunction

function! s:do_download() abort
  if !exists('s:current_version')
    if executable(s:prebuilt_maple_binary)
      "Since v0.7
      let version_line = system(s:prebuilt_maple_binary.' version')
      let s:current_version = str2nr(matchstr(version_line, '0.1.\zs\d\+'))
    else
      let s:current_version = -1
    endif
  endif
  " Since v0.14 maple itself is able to download the latest release binary.
  if s:current_version >= 14
    let cmd = s:prebuilt_maple_binary.' upgrade --download'
    call s:run_term(cmd, s:plugin_root_dir, 'download the latest prebuilt maple binary successfully')
  else
    call clap#installer#download_binary()
  endif
endfunction

function! clap#installer#force_download() abort
  call s:do_download()
endfunction

function! clap#installer#install(try_download) abort
  if clap#job#daemon#is_running()
    call clap#job#daemon#stop()
  endif

  " Always prefer to compile it locally.
  if executable('cargo')
    call clap#installer#build_all()
  " People are willing to use the prebuilt binary
  elseif a:try_download
    call s:do_download()
  else
    call clap#helper#echo_warn('Skipped, cargo does not exist and no prebuilt binary downloaded.')
  endif
endfunction

function! s:unix_sanity_check_is_ok() abort
  " If &shell is not set properly, everything will fail
  if executable(&shell) != 1
    call clap#helper#echo_error('Shell not executable. Check if '. &shell . 'exists!')
    return 0
  endif
  " If on *BSD, we need to invoke gmake for clap's Makefiles
  if !has('win32')
    if !executable('uname')
      call clap#helper#echo_error('uname failed! Cannot detect OS!')
      return 0
    endif
    let l:uname = substitute(system('uname'), '\n', '', '')
    if l:uname ==? 'FreeBSD' || l:uname ==? 'OpenBSD'
      let s:make_cmd = 'gmake'
      if executable(s:make_cmd) != 1
        call clap#helper#echo_error('To set up clap binaries you need to install gmake package.')
        return 0
      endif
      let s:rust_ext_cmd = s:make_cmd . ' build'
    endif
  endif
  return 1
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
