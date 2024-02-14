" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Change state of current filtering, e.g., matches count.

let s:save_cpo = &cpoptions
set cpoptions&vim

function! s:unlet_vars(vars) abort
  for var in a:vars
    if exists(var)
      execute 'unlet' var
    endif
  endfor
endfunction

function! s:remove_provider_tmp_vars(vars) abort
  for var in a:vars
    if has_key(g:clap.provider, var)
      call remove(g:clap.provider, var)
    endif
  endfor
endfunction

" Clear the previous temp state when invoking a new provider.
function! clap#state#clear_pre() abort
  call s:unlet_vars([
        \ 'g:__clap_provider_cwd',
        \ 'g:__clap_provider_did_sink',
        \ 'g:__clap_forerunner_result',
        \ 'g:__clap_match_scope_enum',
        \ 'g:__clap_recent_files_dyn_tmp',
        \ 'g:__clap_forerunner_tempfile',
        \ ])
  let g:clap.display.initial_size = -1
  let g:__clap_icon_added_by_maple = v:false
  call clap#indicator#reset()
endfunction

" Clear temp state on clap#_exit_provider()
function! clap#state#clear_post() abort
  call s:remove_provider_tmp_vars([
        \ 'args',
        \ 'source_tempfile',
        \ ])

  call s:unlet_vars([
        \ 'g:__clap_fuzzy_matched_indices',
        \ 'g:__clap_lines_truncated_map',
        \ ])

  call map(g:clap.tmps, 'delete(v:val)')
  let g:clap.tmps = []
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
