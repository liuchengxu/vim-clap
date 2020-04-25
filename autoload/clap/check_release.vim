function! clap#check_release#check(download) abort
  let maple_bin = clap#maple#binary()
  let maple_bin = expand('~/.vim/plugged/vim-clap/bin/maple-local')
  if a:download
    let check_result = systemlist(printf('"%s" check-release --download', maple_bin))
  else
    let check_result = systemlist(printf('"%s" check-release', maple_bin))
  endif
  if !v:shell_error
    call clap#helper#echo_warn(check_result[0])
  else
    echoerr printf('error happened for %s check-release', maple_bin)
  endif
endfunction

function! clap#check_release#try_upgrade() abort
  " if !clap#maple#using_prebuilt_binary()
    " call clap#helper#echo_error(':Clap upgrade-binary is only meaningful for these using the prebuilt binary')
    " return
  " endif
  call clap#check_release#check(v:false)
endfunction
