let s:layout_keys = ['width', 'height', 'row', 'col', 'relative', 'win']

function! s:validate_layout(layout) abort
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

function! clap#layout#calc() abort
  if exists('g:clap_layout')
    call s:validate_layout(g:clap_layout)
    if g:clap_layout.relative ==# 'win'
      let width = winwidth(g:clap.start.winid)
      let height = winheight(g:clap.start.winid)
      let opts = {'relative': 'win', 'win': g:clap.start.winid}
    else
      let width = &columns
      let height = &lines
      let opts = {'relative': 'editor'}
    endif
    return extend(opts, {
          \ 'width': s:calc(width, g:clap_layout.width),
          \ 'height': s:calc(height, g:clap_layout.height),
          \ 'row': s:calc(height, g:clap_layout.row),
          \ 'col': s:calc(width, g:clap_layout.col),
          \ })
  else
    return {
          \ 'width': &columns * 2 / 3,
          \ 'height': &lines  * 1 / 3,
          \ 'row': &lines / 3 - 1,
          \ 'col': &columns * 2 / 3 / 4,
          \ 'relative': 'editor',
          \ }
  endif
endfunction
