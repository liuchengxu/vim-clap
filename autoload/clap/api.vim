" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Make a compatible layer between neovim and vim.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:is_nvim = has('nvim')

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

if exists('*win_execute')
  function! clap#api#win_execute(winid, command) abort
    return win_execute(a:winid, a:command)
  endfunction
else
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
endif

if s:is_nvim
  function! clap#api#floating_win_is_valid(winid) abort
    return nvim_win_is_valid(a:winid)
  endfunction
else
  function! clap#api#floating_win_is_valid(winid) abort
    return !empty(popup_getpos(a:winid))
  endfunction
endif

let s:api = {}

function! s:api.context_query_or_input() abort
  return has_key(g:clap.context, 'query') ? g:clap.context.query : g:clap.input.get()
endfunction

" The leading icon is stripped.
function! s:api.display_getcurline() abort
  return [g:clap.display.getcurline(), get(g:, '__clap_icon_added_by_maple', v:false)]
endfunction

function! s:api.provider_source() abort
  if has_key(g:clap.provider, 'source_type') && has_key(g:clap.provider._(), 'source')
    if g:clap.provider.source_type == g:__t_string
      return [g:clap.provider._().source]
    elseif g:clap.provider.source_type == g:__t_func_string
      return [g:clap.provider._().source()]
    elseif g:clap.provider.source_type == g:__t_list
      return [g:clap.provider._().source]
    elseif g:clap.provider.source_type == g:__t_func_list
      " Note that this function call should always be pretty fast and not slow down Vim.
      return [g:clap.provider._().source()]
    endif
  endif
  return []
endfunction

function! s:api.provider_source_cmd() abort
  if has_key(g:clap.provider, 'source_type') && has_key(g:clap.provider._(), 'source')
    if g:clap.provider.source_type == g:__t_string
      return [g:clap.provider._().source]
    elseif g:clap.provider.source_type == g:__t_func_string
      return [g:clap.provider._().source()]
    endif
  endif
  return []
endfunction

function! s:api.provider_args() abort
  return get(g:clap.provider, 'args', [])
endfunction

function! s:api.provider_raw_args() abort
  return get(g:clap.provider, 'raw_args', [])
endfunction

function! s:api.input_set(value) abort
  call g:clap.input.set(a:value)
endfunction

function! s:api.get_var(var) abort
  return get(g:, a:var, v:null)
endfunction

function! s:api.set_var(name, value) abort
  execute 'let '.a:name.'= a:value'
endfunction

function! s:api.current_buffer_path() abort
  return expand('#'.bufnr('%').':p')
endfunction

function! s:api.matchdelete_batch(match_ids, winid) abort
  call map(a:match_ids, 'matchdelete(v:val, a:winid)')
endfunction

function! clap#api#call(method, args) abort
  " Catch all the exceptions
  try
    if has_key(s:api, a:method)
      return call(s:api[a:method], a:args)
    else
      return call(a:method, a:args)
    endif
  catch /^Vim:Interrupt$/ " catch interrupts (CTRL-C)
  catch
    echoerr printf('[clap#api#call] method: %s, args: %s, exception: %s', a:method, string(a:args), v:exception)
  endtry
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
