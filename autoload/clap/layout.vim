let s:is_nvim = has('nvim')
let s:layout_keys = ['width', 'height', 'row', 'col', 'relative', 'win']
let s:default_layout = {
          \ 'width': &columns * 2 / 3,
          \ 'height': &lines  * 1 / 3,
          \ 'row': &lines / 3 - 1,
          \ 'col': &columns * 2 / 3 / 4,
          \ }

function! s:validate(layout) abort
  for key in keys(a:layout)
    if index(s:layout_keys, key) < 0
      call g:clap.abort('Invalid entry in g:clap_layout:'.key)
    endif
  endfor
endfunction

function! s:calc(origin, size) abort
  if type(a:size) == v:t_number
    return a:size
  elseif a:size =~# '%$'
    return eval(a:size[:-2].'*'.a:origin.'/100')
  else
    call g:clap.abort('Invalid value for g:clap_layout')
  endif
endfunction

function! s:get_ref_size() abort
  if g:clap_layout.relative ==# 'win'
    return [winwidth(g:clap.start.winid), winheight(g:clap.start.winid)]
  else
    return [&columns, &lines]
  endif
endfunction

if s:is_nvim
  function! clap#layout#calc() abort
    if exists('g:clap_layout')
      call s:validate(g:clap_layout)
      let [width, height] = s:get_ref_size()
      if g:clap_layout.relative ==# 'win'
        let opts = {'relative': 'win', 'win': g:clap.start.winid}
      else
        let opts = {'relative': 'editor'}
      endif
      return extend(opts, {
            \ 'width': s:calc(width, g:clap_layout.width),
            \ 'height': s:calc(height, g:clap_layout.height),
            \ 'row': s:calc(height, g:clap_layout.row),
            \ 'col': s:calc(width, g:clap_layout.col),
            \ })
    else
      return extend({'relative': 'editor'}, s:default_layout)
    endif
  endfunction
else
  function! clap#layout#calc() abort
    if exists('g:clap_layout')
      if g:clap_layout.relative ==# 'win'
        let [row, col] = win_screenpos(g:clap.start.winid)
        let width = winwidth(g:clap.start.winid)
        let height = winheight(g:clap.start.winid)
      else
        let [row, col] = [0, 0]
        let width = &columns
        let height = &lines
      endif
      return {
            \ 'width': s:calc(width, g:clap_layout.width),
            \ 'height': s:calc(height, g:clap_layout.height),
            \ 'row': s:calc(height, g:clap_layout.row) + row,
            \ 'col': s:calc(width, g:clap_layout.col) + col,
            \ }
    else
      return s:default_layout
    endif
  endfunction
endif

function! clap#layout#on_resize() abort
endfunction

if s:is_nvim
  function! clap#layout#on_resize() abort
    " FIXME resize if vim-clap is visible
    call clap#floating_win#reconfigure_display_opts()
  endfunction
else
  function! clap#layout#on_resize() abort
    call clap#popup#reconfigure_display_opts()
  endfunction
endif
