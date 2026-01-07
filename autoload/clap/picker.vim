" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Picker management.

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

function! clap#picker#init(lines, truncated_map, icon_added, using_cache) abort
  if !g:clap.display.win_is_valid()
    return
  endif
  if empty(g:clap.input.get())
    call g:clap.display.set_lines_lazy(a:lines)
    call g:clap#display_win.shrink_if_undersize()
  endif

  if a:using_cache
    let g:__clap_current_forerunner_status = g:clap_forerunner_status_sign.using_cache
  else
    let g:__clap_current_forerunner_status = g:clap_forerunner_status_sign.done
  endif

  let g:__clap_icon_added_by_maple = a:icon_added
  if !empty(a:truncated_map)
    let g:__clap_lines_truncated_map = a:truncated_map
  elseif exists('g:__clap_lines_truncated_map')
    unlet g:__clap_lines_truncated_map
  endif

  call clap#indicator#update_processed(g:clap.display.initial_size)
  call clap#sign#ensure_exists()
  call clap#spinner#refresh()
  call clap#preview#update_with_delay()

  " Apply path prefix dimming for file-related providers
  call clap#highlighter#path#apply()
endfunction

function! clap#picker#process_progress(matched, processed) abort
  call clap#indicator#update(a:matched, a:processed)
endfunction

function! clap#picker#update(update_info) abort
  if !g:clap.display.win_is_valid()
    return
  endif

  let update_info = a:update_info

  call clap#indicator#update(update_info.matched, update_info.processed)

  if update_info.matched == 0
    call g:clap.display.clear_highlight()
    call g:clap.display.set_lines([g:clap_no_matches_msg])
    call g:clap.preview.clear()
    if exists('g:__clap_lines_truncated_map')
      unlet g:__clap_lines_truncated_map
    endif
    return
  else
    call g:clap.display.set_lines(update_info.lines)
  endif

  if has_key(update_info, 'truncated_map')
    let g:__clap_lines_truncated_map = update_info.truncated_map
  elseif exists('g:__clap_lines_truncated_map')
    unlet g:__clap_lines_truncated_map
  endif

  let g:__clap_icon_added_by_maple = get(update_info, 'icon_added', v:false)

  if has_key(update_info, 'display_syntax')
    call setbufvar(g:clap.display.bufnr, '&syntax', update_info.display_syntax)
  endif

  call clap#sign#ensure_exists()

  if has_key(update_info, 'preview') && update_info.preview isnot v:null
    if !empty(update_info.preview)
      call clap#picker#update_preview(update_info.preview)
    endif
  else
    call clap#preview#update_with_delay()
  endif

  if has_key(update_info, 'indices')
    try
      call clap#highlighter#add_highlights(update_info.indices)
    catch
      return
    endtry
  endif

  " Apply path prefix dimming for file-related providers
  call clap#highlighter#path#apply()
endfunction

function! clap#picker#update_on_empty_query(lines, truncated_map, icon_added) abort
  if !g:clap.display.win_is_valid()
    return
  endif
  call g:clap.display.set_lines_lazy(a:lines)
  call g:clap#display_win.shrink_if_undersize()
  if !empty(a:truncated_map)
    let g:__clap_lines_truncated_map = a:truncated_map
  elseif exists('g:__clap_lines_truncated_map')
    unlet g:__clap_lines_truncated_map
  endif
  let g:__clap_icon_added_by_maple = a:icon_added
  call clap#sign#ensure_exists()
  call g:clap.display.clear_highlight()
  call clap#indicator#update_matched(0)
  call clap#preview#update_with_delay()

  " Apply path prefix dimming for file-related providers
  call clap#highlighter#path#apply()
endfunction

function! clap#picker#update_preview(preview) abort
  if !g:clap.display.win_is_valid()
    return
  endif
  if has_key(a:preview, 'lines')
    try
      call g:clap.preview.show(a:preview.lines)
    catch
      " Neovim somehow has a bug decoding the lines
      call g:clap.preview.show(['Error occurred while showing the preview:', v:exception, '', string(a:preview.lines)])
      return
    endtry
    if has_key(a:preview, 'sublime_syntax_highlights')
      for [lnum, line_highlight] in a:preview.sublime_syntax_highlights
        try
          call clap#highlighter#highlight_line(g:clap.preview.bufnr, lnum, line_highlight)
        catch
          " Ignore any potential errors as the line might be truncated.
        endtry
      endfor
    elseif has_key(a:preview, 'tree_sitter_highlights')
      call clap#highlighter#add_ts_highlights(g:clap.preview.bufnr, [], a:preview.tree_sitter_highlights)
    elseif has_key(a:preview, 'vim_syntax_info')
      let vim_syntax_info = a:preview.vim_syntax_info
      if !empty(vim_syntax_info.syntax)
        call g:clap.preview.set_syntax(vim_syntax_info.syntax)
      elseif !empty(vim_syntax_info.fname)
        call g:clap.preview.set_syntax(clap#ext#into_filetype(vim_syntax_info.fname))
      endif
    endif
    call clap#preview#highlight_header()

    if has_key(a:preview, 'highlight_line')
      let highlight_line = a:preview.highlight_line
      if has_key(highlight_line, 'column_range')
        call g:clap.preview.add_highlight(highlight_line)
      else
        call g:clap.preview.add_highlight(highlight_line.line_number)
      endif
    endif

    if has_key(a:preview, 'scrollbar')
      let [top_position, length] = a:preview.scrollbar
      call clap#floating_win#show_preview_scrollbar(top_position, length)
    endif
  endif
endfunction

function! clap#picker#clear_preview() abort
  call g:clap.preview.clear()
endfunction

function! clap#picker#clear_all() abort
  if exists('g:__clap_lines_truncated_map')
    unlet g:__clap_lines_truncated_map
  endif
  call g:clap.display.clear()
  call g:clap.preview.clear()
  call clap#indicator#set_none()
endfunction

function! clap#picker#set_input(new) abort
  call g:clap.input.set(a:new)
  " Move cursor to the end of line.
  call clap#api#win_execute(g:clap.input.winid, 'call cursor(1, 1000)')
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
function! clap#picker#clear_state_pre() abort
  call s:unlet_vars([
        \ 'g:__clap_provider_cwd',
        \ 'g:__clap_provider_did_sink',
        \ 'g:__clap_forerunner_result',
        \ 'g:__clap_remote_sink_triggered',
        \ 'g:__clap_match_scope_enum',
        \ 'g:__clap_recent_files_dyn_tmp',
        \ 'g:__clap_forerunner_tempfile',
        \ ])
  let g:clap.display.initial_size = -1
  let g:__clap_icon_added_by_maple = v:false
  call clap#indicator#reset()
endfunction

" Clear temp state on clap#_exit_provider()
function! clap#picker#clear_state_post() abort
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
