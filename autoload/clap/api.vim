" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Make a compatible layer between neovim and vim.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:is_nvim = has('nvim')

function! clap#api#setbufvar_batch(bufnr, dict) abort
  call map(a:dict, 'setbufvar(a:bufnr, v:key, v:val)')
endfunction

" If the user has specified the externalfilter option in the context.
" If so, we should not use the built-in fuzzy filter then.
function! clap#api#has_externalfilter() abort
  return has_key(g:clap.context, 'ef')
        \ || has_key(g:clap.context, 'externalfilter')
endfunction

" Returns the original full line with icon if it was added by maple given
" the lnum of display buffer.
function! clap#api#get_origin_line_at(lnum) abort
  if exists('g:__clap_lines_truncated_map')
        \ && has_key(g:__clap_lines_truncated_map, a:lnum)
    return g:__clap_lines_truncated_map[a:lnum]
  else
    return get(getbufline(g:clap.display.bufnr, a:lnum), 0, '')
  endif
endfunction

if s:is_nvim
  function! clap#api#win_execute(winid, command) abort
    let cur_winid = bufwinid('')
    if cur_winid != a:winid
      noautocmd call win_gotoid(a:winid)
      try
        return execute(a:command)
      finally
        noautocmd call win_gotoid(cur_winid)
      endtry
    else
      return execute(a:command)
    endif
  endfunction

  function! clap#api#floating_win_is_valid(winid) abort
    return nvim_win_is_valid(a:winid)
  endfunction
else
  function! clap#api#win_execute(winid, command) abort
    return win_execute(a:winid, a:command)
  endfunction

  function! clap#api#floating_win_is_valid(winid) abort
    return !empty(popup_getpos(a:winid))
  endfunction
endif

let &cpoptions = s:save_cpo
unlet s:save_cpo
