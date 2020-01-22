" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Custom window layout.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:is_nvim = has('nvim')
let s:layout_keys = ['width', 'height', 'row', 'col', 'relative']
let s:default_layout = {
          \ 'width': '67%',
          \ 'height': '33%',
          \ 'row': '33%',
          \ 'col': '17%',
          \ }

if s:is_nvim
  call add(s:layout_keys, 'win')
  let s:default_layout.relative = 'editor'
endif

function! s:validate(layout) abort
  for key in keys(a:layout)
    if index(s:layout_keys, key) < 0
      call g:clap.abort('Invalid entry: '.key.' for g:clap_layout')
    endif
  endfor
endfunction

function! s:calc(origin, size) abort
  if type(a:size) == v:t_number
    return a:size
  elseif a:size =~# '%$'
    return eval(a:size[:-2].'*'.a:origin.'/100')
  else
    call g:clap.abort(printf('Invalid value %s for g:clap_layout, allowed: Number or "Number%"', a:size))
  endif
endfunction

if s:is_nvim
  function! s:user_layout() abort
    let layout = extend(copy(s:default_layout), g:clap_layout)
    if has_key(layout, 'relative') && layout.relative ==# 'editor'
      let [width, height] = [&columns, &lines]
      let opts = {'relative': 'editor'}
    else
      let [width, height] = [winwidth(g:clap.start.winid), winheight(g:clap.start.winid)]
      let opts = {'relative': 'win', 'win': g:clap.start.winid}
    endif

    return extend(opts, {
          \ 'width': s:calc(width, layout.width),
          \ 'height': s:calc(height, layout.height),
          \ 'row': s:calc(height, layout.row),
          \ 'col': s:calc(width, layout.col),
          \ })
  endfunction

  function! s:calc_default() abort
    let [width, height] = [winwidth(g:clap.start.winid), winheight(g:clap.start.winid)]
    return {
          \ 'width': s:calc(width, s:default_layout.width),
          \ 'height': s:calc(height, s:default_layout.height),
          \ 'row': s:calc(height, s:default_layout.row),
          \ 'col': s:calc(width, s:default_layout.col),
          \ 'win': g:clap.start.winid,
          \ 'relative': 'win',
          \ }
  endfunction
else
  function! s:user_layout() abort
    let layout = extend(copy(s:default_layout), g:clap_layout)
    if has_key(layout, 'relative') && layout.relative ==# 'editor'
      let [row, col] = [0, 0]
      let width = &columns
      let height = &lines
    else
      let [row, col] = win_screenpos(g:clap.start.winid)
      let width = winwidth(g:clap.start.winid)
      let height = winheight(g:clap.start.winid)
    endif
    return {
          \ 'width': s:calc(width, layout.width),
          \ 'height': s:calc(height, layout.height),
          \ 'row': s:calc(height, layout.row) + row,
          \ 'col': s:calc(width, layout.col) + col,
          \ }
  endfunction

  function! s:calc_default() abort
    let [width, height] = [winwidth(g:clap.start.winid), winheight(g:clap.start.winid)]
    let [row, col] = win_screenpos(g:clap.start.winid)
    return {
          \ 'width': s:calc(width, s:default_layout.width),
          \ 'height': s:calc(height, s:default_layout.height),
          \ 'row': s:calc(height, s:default_layout.row) + row,
          \ 'col': s:calc(width, s:default_layout.col) + col,
          \ }
  endfunction
endif

function! clap#layout#calc() abort
  if exists('g:clap_layout')
    call s:validate(g:clap_layout)
    return s:user_layout()
  else
    return s:calc_default()
  endif
endfunction

function! clap#layout#on_resized() abort
  " FIXME resize window if vim-clap is visible
  " The easiest way is to close and reopen vim-clap, so I'll leave this for
  " now.
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
