" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Picker management.

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

function! clap#picker#update(decoded_msg) abort
  if !g:clap.display.win_is_valid()
    return
  endif

  let decoded = a:decoded_msg

  if has_key(decoded, 'matched')
    if has_key(decoded, 'processed')
      call clap#indicator#update(decoded.matched, decoded.processed)
    else
      call clap#indicator#update_matched(decoded.matched)
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
  elseif exists('g:__clap_lines_truncated_map')
    unlet g:__clap_lines_truncated_map
  endif

  if has_key(decoded, 'icon_added')
    let g:__clap_icon_added_by_maple = decoded.icon_added
  endif

  if has_key(decoded, 'display_syntax')
    call setbufvar(g:clap.display.bufnr, '&syntax', decoded.display_syntax)
  endif

  call clap#sign#ensure_exists()

  if has_key(decoded, 'indices')
    try
      call clap#highlighter#add_highlights(decoded.indices)
    catch
      return
    endtry
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
