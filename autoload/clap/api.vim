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

function! s:api.display_getcurline() abort
  return g:clap.display.getcurline()
endfunction

function! s:api.display_getcurlnum() abort
  return g:clap.display.getcurlnum()
endfunction

function! s:api.input_get() abort
  return g:clap.input.get()
endfunction

function! s:api.get_var(var) abort
  return get(g:, a:var, v:null)
endfunction

function! s:api.set_var(name, value) abort
  execute 'let '.a:name.'= a:value'
endfunction

function! s:api.working_dir() abort
  return clap#rooter#working_dir()
endfunction

function! s:api.provider_id() abort
  return g:clap.provider.id
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

function! s:api.context_query_or_input() abort
  return has_key(g:clap.context, 'query') ? g:clap.context.query : g:clap.input.get()
endfunction

function! s:api.bufname(bufnr) abort
  return bufname(a:bufnr)
endfunction

function! s:api.fnamemodify(bufname, mods) abort
  return fnamemodify(a:bufname, a:mods)
endfunction

function! clap#api#call(method, args) abort
  if has_key(s:api, a:method)
    return call(s:api[a:method], a:args)
  else
    return call(a:method, a:args)
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
