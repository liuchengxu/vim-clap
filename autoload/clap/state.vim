" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Change state of current filtering, e.g., matches count.

let s:save_cpo = &cpoptions
set cpoptions&vim

" NOTE: some local variable without explicit l:, e.g., count,
" may run into some erratic read-only error.
function! clap#state#refresh_matches_count(cnt) abort
  call clap#indicator#update_matched(a:cnt)
  call clap#sign#reset_to_first_line()
endfunction

function! clap#state#process_filter_message(decoded_msg, ensure_sign_exists) abort
  if !g:clap.display.win_is_valid()
    return
  endif

  let decoded = a:decoded_msg

  if has_key(decoded, 'total')
    if decoded.total == 0 && exists('g:__clap_lines_truncated_map')
      unlet g:__clap_lines_truncated_map
    endif
    call clap#indicator#update_matched(decoded.total)
  endif

  if has_key(decoded, 'matched')
    call clap#indicator#update(decoded.matched, decoded.processed)
  elseif has_key(decoded, 'total_matched')
    if has_key(decoded, 'total_processed')
      call clap#indicator#update(decoded.total_matched, decoded.total_processed)
    else
      call clap#indicator#update_matched(decoded.total_matched)
    endif
  endif

  if has_key(decoded, 'lines')
    call g:clap.display.set_lines(decoded.lines)
    if empty(decoded.lines)
      call g:clap.preview.clear()
      return
    endif
  endif

  if exists('g:__clap_lines_truncated_map')
    unlet g:__clap_lines_truncated_map
  endif

  if has_key(decoded, 'truncated_map')
    let g:__clap_lines_truncated_map = decoded.truncated_map
  elseif exists('g:__clap_lines_truncated_map')
    unlet g:__clap_lines_truncated_map
  endif

  if has_key(decoded, 'icon_added')
    let g:__clap_icon_added_by_maple = decoded.icon_added
  endif

  if has_key(decoded, 'display_syntax')
    call setbufvar(g:clap.display.bufnr, '&syntax', decoded.display_syntax)
  endif

  if a:ensure_sign_exists
    call clap#sign#ensure_exists()
  endif

  if has_key(decoded, 'indices')
    try
      call clap#highlighter#add_highlights(decoded.indices)
    catch
      return
    endtry
  endif
endfunction

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
