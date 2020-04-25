function! clap#check_release#check(download) abort
  " if !clap#maple#using_prebuilt_binary()
    " echoerr 'check-release is only used for users that are using the prebuilt binary'
    " return
  " endif
  let maple_bin = clap#maple#binary()
  let maple_bin = expand('~/.vim/plugged/vim-clap/bin/maple-local')
  if a:download
    let check_result = system('"%s" check-release --download', maple_bin)
  else
    let check_result = system('"%s" check-release', maple_bin)
  endif
  if !v:shell_error
    echom check_result
  else
    echoerr printf('error happened for %s check-release', maple_bin).v:exception
  endif
endfunction
