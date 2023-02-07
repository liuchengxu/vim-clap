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

  if has_key(decoded, 'truncated_map')
    let g:__clap_lines_truncated_map = decoded.truncated_map
  endif

  if has_key(decoded, 'icon_added')
    let g:__clap_icon_added_by_maple = decoded.icon_added
  endif

  if a:ensure_sign_exists
    call clap#sign#ensure_exists()
  endif

  if has_key(decoded, 'indices')
    try
      call clap#highlight#add_highlights_with_delay(decoded.indices)
    catch
      return
    endtry
  endif
endfunction

function! clap#state#process_progress(matched, processed) abort
  call clap#indicator#update(a:matched, a:processed)
endfunction

function! clap#state#process_progress_full(display_lines, matched, processed) abort
  if !g:clap.display.win_is_valid()
    return
  endif
  call clap#indicator#update(a:matched, a:processed)
  call g:clap.display.set_lines(a:display_lines.lines)
  call clap#highlight#add_highlights_with_delay(a:display_lines.indices)
  call clap#preview#update_with_delay()
  if a:matched > 0
    call clap#sign#ensure_exists()
  endif
  let g:__clap_icon_added_by_maple = a:display_lines.icon_added
  if !empty(a:display_lines.truncated_map)
    let g:__clap_lines_truncated_map = a:display_lines.truncated_map
  endif
endfunction

function! clap#state#render_preview(preview) abort
  if has_key(a:preview, 'lines')
    try
      call g:clap.preview.show(a:preview.lines)
    catch
      " Neovim somehow has a bug decoding the lines
      call g:clap.preview.show(['Error occurred while showing the preview:', v:exception, '', string(a:preview.lines)])
      return
    endtry
    if has_key(a:preview, 'syntax')
      call g:clap.preview.set_syntax(a:preview.syntax)
    elseif has_key(a:preview, 'fname')
      call g:clap.preview.set_syntax(clap#ext#into_filetype(a:preview.fname))
    endif
    call clap#preview#highlight_header()

    if has_key(a:preview, 'hi_lnum')
      call g:clap.preview.add_highlight(a:preview.hi_lnum+1)
    endif
  endif
endfunction

function! clap#state#process_response_on_typed(result) abort
  if has_key(a:result, 'matched')
    if has_key(a:result, 'processed')
      call clap#indicator#update(a:result.matched, a:result.processed)
      if a:result.processed == 0
        call clap#state#clear_screen()
        return
      endif
    else
      call clap#indicator#update_matched_only(a:result.matched)
    endif
  else
    " Should be unreachable.
    return
  endif

  if has_key(a:result, 'truncated_map')
    let g:__clap_lines_truncated_map = a:result.truncated_map
  endif

  if has_key(a:result, 'icon_added')
    let g:__clap_icon_added_by_maple = a:result.icon_added
  endif

  call g:clap.display.set_lines(a:result.lines)
  call clap#highlight#add_highlights_with_delay(a:result.indices)
  call clap#sign#ensure_exists()
  call clap#preview#update_with_delay()

  if has_key(a:result, 'preview')
    if !empty(a:result.preview)
      call clap#state#render_preview(a:result.preview)
    endif
  else
    call clap#preview#update_with_delay()
  endif
endfunction

function! clap#state#clear_screen() abort
  if exists('g:__clap_lines_truncated_map')
    unlet g:__clap_lines_truncated_map
  endif
  call g:clap.display.clear()
  call g:clap.preview.clear()
  call clap#indicator#set_none()
endfunction

function! clap#state#clear_preview() abort
  call g:clap.preview.clear()
endfunction

function! clap#state#init_display(lines, truncated_map, icon_added, using_cache) abort
  if !g:clap.display.win_is_valid()
    return
  endif
  if empty(g:clap.input.get())
    if g:clap.provider.id ==# 'blines'
      call clap#provider#blines#initialize(a:lines)
    else
      call g:clap.display.set_lines_lazy(a:lines)
    endif
    call g:clap#display_win.shrink_if_undersize()
  endif

  let g:__clap_icon_added_by_maple = a:icon_added
  if !empty(a:truncated_map)
    let g:__clap_lines_truncated_map = a:truncated_map
  endif

  if a:using_cache
    let g:__clap_current_forerunner_status = g:clap_forerunner_status_sign.using_cache
  else
    let g:__clap_current_forerunner_status = g:clap_forerunner_status_sign.done
  endif

  call clap#indicator#update_processed(g:clap.display.initial_size)
  call clap#sign#ensure_exists()
  call clap#spinner#refresh()
  call clap#preview#update_with_delay()
endfunction

function! clap#state#update_on_empty_query(lines, truncated_map, icon_added) abort
  if !g:clap.display.win_is_valid()
    return
  endif
  call g:clap.display.set_lines_lazy(a:lines)
  call g:clap#display_win.shrink_if_undersize()
  if !empty(a:truncated_map)
    let g:__clap_lines_truncated_map = a:truncated_map
  endif
  let g:__clap_icon_added_by_maple = a:icon_added

  call clap#sign#ensure_exists()

  call clap#indicator#update_matched(0)
  call clap#preview#update_with_delay()
endfunction

function! clap#state#set_input(new) abort
  call g:clap.input.set(a:new)
  " Move cursor to the end of line.
  call clap#api#win_execute(g:clap.input.winid, 'call cursor(1, 1000)')
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
        \ 'g:__clap_provider_cwd',
        \ 'g:__clap_forerunner_result',
        \ 'g:__clap_match_scope_enum',
        \ 'g:__clap_recent_files_dyn_tmp',
        \ 'g:__clap_forerunner_tempfile',
        \ ])
  let g:clap.display.initial_size = -1
  let g:__clap_icon_added_by_maple = v:false
  call clap#indicator#reset()
endfunction

" Clear temp state on clap#_exit()
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
