" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Initialize the plugin, including making a compatible API layer
" and flexiable highlight groups.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:is_nvim = has('nvim')

if s:is_nvim
  function! s:reconfigure_display_opts() abort
    call clap#floating_win#reconfigure_display_opts()
  endfunction
else
  function! s:reconfigure_display_opts() abort
    call clap#popup#reconfigure_display_opts()
  endfunction
endif

function! clap#init#() abort
  call clap#api#bake()
  call clap#themes#init_hi_groups()

  " This augroup should be retained after closing vim-clap for the benefit
  " of next run.
  if !exists('#ClapResize')
    augroup ClapResize
      autocmd!
      autocmd VimResized * call clap#layout#on_resized()
    augroup END
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
