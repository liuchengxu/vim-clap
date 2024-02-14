" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Picker management.

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

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

  let g:__clap_icon_added_by_maple = update_info.icon_added

  if has_key(update_info, 'display_syntax')
    call setbufvar(g:clap.display.bufnr, '&syntax', update_info.display_syntax)
  endif

  call clap#sign#ensure_exists()

  call clap#preview#update_with_delay()

  if has_key(update_info, 'indices')
    try
      call clap#highlighter#add_highlights(update_info.indices)
    catch
      return
    endtry
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
