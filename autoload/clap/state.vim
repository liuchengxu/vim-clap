" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Change state of current filtering, e.g., matches count.

let s:save_cpo = &cpoptions
set cpoptions&vim

" NOTE: some local variable without explicit l:, e.g., count,
" may run into some erratic read-only error.
function! clap#state#refresh_matches_count(cnt) abort
  call clap#indicator#set_matches_number(a:cnt)
  call clap#sign#reset_to_first_line()
endfunction

function! clap#state#handle_message(msg) abort
  let decoded = json_decode(a:msg)

  if has_key(decoded, 'total')
    call clap#indicator#set_matches_number(decoded.total)
  endif

  if has_key(decoded, 'lines')
    call g:clap.display.set_lines(decoded.lines)
  endif

  if has_key(decoded, 'truncated_map')
    let g:__clap_lines_truncated_map = decoded.truncated_map
  endif

  call clap#sign#ensure_exists()

  if has_key(decoded, 'indices')
    try
      call clap#highlight#add_fuzzy_async_with_delay(decoded.indices)
    catch
      return
    endtry
  endif
endfunction

" Returns the cached source tmp file.
"
" Write the providers whose `source` is list-style into a tempfile.
function! clap#state#into_tempfile(source_list) abort
  if has_key(g:clap.provider, 'source_tempfile')
    let tmp = g:clap.provider.source_tempfile
    return tmp
  else
    let tmp = tempname()
    if writefile(a:source_list, tmp) == 0
      call add(g:clap.tmps, tmp)
      let g:clap.provider.source_tempfile = tmp
      return tmp
    else
      call g:clap.abort('Fail to write source to a temp file')
      return ''
    endif
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
        \ 's:current_matches',
        \ 'g:__clap_raw_source',
        \ 'g:__clap_provider_cwd',
        \ 'g:__clap_forerunner_result',
        \ 'g:__clap_initial_source_size',
        \ 'g:__clap_builtin_line_splitter_enum',
        \ ])
  call clap#indicator#clear()
  if exists('g:__clap_forerunner_tempfile')
    unlet g:__clap_forerunner_tempfile
  endif
endfunction

" Clear temp state on clap#_exit()
function! clap#state#clear_post() abort
  call s:remove_provider_tmp_vars([
        \ 'args',
        \ 'source_tempfile',
        \ 'should_switch_to_async',
        \ ])

  call s:unlet_vars([
        \ 'g:__clap_fuzzy_matched_indices',
        \ 'g:__clap_lines_truncated_map',
        \ ])
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
