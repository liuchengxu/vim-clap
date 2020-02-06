" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Run function given the specified working directory.

let s:save_cpo = &cpoptions
set cpoptions&vim

" Some providers may change the cwd via the passed option, e.g., Clap files
" and Clap grep.
"
" Skip if g:__clap_provider_cwd already exists as it only has be done once in
" each provider context.
function! clap#rooter#try_set_cwd() abort
  if !exists('g:__clap_provider_cwd') && !empty(g:clap.provider.args)
    let dir = g:clap.provider.args[-1]
    if isdirectory(expand(dir))

      " dir could be a relative directory, e.g., ..
      " We must use the absolute directory for g:__clap_provider_cwd,
      " otherwise s:run_from_target_dir could `lcd ..` multiple times.
      let save_cwd = getcwd()
      noautocmd execute 'lcd' dir
      let g:__clap_provider_cwd = getcwd()
      noautocmd execute 'lcd' save_cwd

      let g:clap.provider.args = g:clap.provider.args[:-2]
    endif
  endif
endfunction

function! clap#rooter#working_dir() abort
  if exists('g:__clap_provider_cwd')
    return g:__clap_provider_cwd
  elseif clap#should_use_raw_cwd()
    return getcwd()
  else
    return clap#path#project_root_or_default(g:clap.start.bufnr)
  endif
endfunction

function! s:run_from_target_dir(target_dir, Run, run_args) abort
  let save_cwd = getcwd()
  try
    execute 'lcd' a:target_dir
    let l:result = call(a:Run, a:run_args)
  catch
    call clap#helper#echo_error(
          \ printf('target_dir:%s, Run:%s, run_args:%s, exception:%s',
          \ a:target_dir,
          \ string(a:Run),
          \ string(a:run_args),
          \ v:exception,
          \ ))
  finally
  " If the sink function changes cwd intentionally? Then we
  " should not restore to the current cwd after executing the sink function.
    if getcwd(winnr()) ==# a:target_dir
      execute 'lcd' save_cwd
    endif
  endtry
  return exists('l:result') ? l:result : []
endfunction

" Argument: Funcref to run as well as its args
function! clap#rooter#run(Run, ...) abort
  if exists('g:__clap_provider_cwd')
    return s:run_from_target_dir(g:__clap_provider_cwd, a:Run, a:000)
  elseif clap#should_use_raw_cwd()
    return call(a:Run, a:000)
  endif

  let project_root = clap#path#find_project_root(g:clap.start.bufnr)

  if empty(project_root)
    " This means to use getcwd()
    let result = call(a:Run, a:000)
  else
    let result = s:run_from_target_dir(project_root, a:Run, a:000)
  endif

  return result
endfunction

" This is used for the sink function.
function! clap#rooter#run_heuristic(Run, ...) abort
  if exists('g:__clap_provider_cwd')
    return s:run_from_target_dir(g:__clap_provider_cwd, a:Run, a:000)
  elseif clap#should_use_raw_cwd()
    return call(a:Run, a:000)
  endif

  let project_root = clap#path#find_project_root(g:clap.start.bufnr)

  if empty(project_root)
    let result = call(a:Run, a:000)

  else

    let save_cwd = getcwd()
    try
      execute 'lcd' project_root
      let l:result = call(a:Run, a:000)
    finally
      " Here we could use a naive heuristic approach to
      " not restore the old cwd when the current working
      " directory is not git root or &autochdir is on.
      " This way is mainly borrowed from fzf.vim.
      if getcwd() ==# project_root && !&autochdir
        execute 'lcd' save_cwd
      endif
    endtry

  endif

  return exists('l:result') ? l:result : []
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
