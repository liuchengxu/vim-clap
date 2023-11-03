" Author: liuchengxu <xuliuchengxlc@gmail.com>

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

if has('nvim')
  let s:colorizer_ns_id = nvim_create_namespace('clap_colorizer')
else
  let s:types = []
endif

function! s:create_new_group(bufnr, highlight_group) abort
  execute printf(
        \ 'hi %s guibg=%s',
        \ a:highlight_group.name,
        \ a:highlight_group.guibg,
        \ )

  if !has('nvim')
    call add(s:types, a:highlight_group.name)
    call prop_type_add(a:highlight_group.name, {'highlight': a:highlight_group.name, 'bufnr': a:bufnr})
  endif
endfunction

" lnum and col is 0-based.
function! s:add_highlight(bufnr, line_number, color_info) abort
  if !hlexists(a:color_info.highlight_group.name)
    call s:create_new_group(a:bufnr, a:color_info.highlight_group)
  endif

  if has('nvim')
    call nvim_buf_add_highlight(a:bufnr, s:colorizer_ns_id,
          \ a:color_info.highlight_group.name,
          \ a:line_number,
          \ a:color_info.col,
          \ a:color_info.col + a:color_info.length,
          \ )
  else
    call prop_add(a:line_number + 1, a:color_info.col + 1, {
          \   'type': a:color_info.highlight_group.name,
          \   'length': a:color_info.length,
          \   'bufnr': a:bufnr,
          \ })
  endif
endfunction

function! clap#plugin#colorizer#add_highlights(bufnr, highlights) abort
  for [line_number, color_infos] in items(a:highlights)
    call map(color_infos, 's:add_highlight(a:bufnr, str2nr(line_number), v:val)')
  endfor
endfunction

function! clap#plugin#colorizer#clear_highlights(bufnr) abort
  if has('nvim')
    call nvim_buf_clear_namespace(a:bufnr, s:colorizer_ns_id, 0, -1)
  else
    call prop_remove({ 'types': s:types, 'bufnr': a:bufnr, 'all': v:true } )
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
