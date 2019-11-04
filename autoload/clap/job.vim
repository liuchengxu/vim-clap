" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: APIs for working with Asynchronous jobs.

function! clap#job#cwd() abort
  if get(g:, 'clap_disable_run_rooter', v:false)
    return getcwd()
  else
    let git_root = clap#util#find_git_root(g:clap.start.bufnr)
    return empty(git_root) ? getcwd() : git_root
  endif
endfunction
