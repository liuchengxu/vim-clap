" vim-clap - Modern interactive filter and dispatcher
" Author:    Liu-Cheng Xu <xuliuchengxlc@gmail.com>
" Website:   https://github.com/liuchengxu/vim-clap
" Version:   0.31
" License:   MIT

if exists('g:loaded_clap')
  finish
endif

let g:loaded_clap = 1

command! -bang -nargs=* -bar -range -complete=customlist,clap#helper#complete Clap call clap#(<bang>0, <f-args>)

let g:__clap_buffers = get(g:, '__clap_buffers', {})

let g:__clap_tab_buffers = get(g:, '__clap_tab_buffers', {})

function! s:OnBufEnter(bufnr) abort
  let tabpagenr = tabpagenr()
  if !has_key(g:__clap_tab_buffers, tabpagenr)
    let g:__clap_tab_buffers[tabpagenr] = []
  endif
  if index(g:__clap_tab_buffers[tabpagenr], a:bufnr) == -1 && bufname('') !=# ''
    call add(g:__clap_tab_buffers[tabpagenr], a:bufnr)
  endif
endfunction

function! s:OnBufDelete(bufnr) abort
  if has_key(g:__clap_buffers, a:bufnr)
    call remove(g:__clap_buffers, a:bufnr)
  endif
  let tabpagenr = tabpagenr()
  if has_key(g:__clap_tab_buffers, tabpagenr)
    let idx = index(g:__clap_tab_buffers[tabpagenr], a:bufnr)
    if idx != -1
      unlet g:__clap_tab_buffers[tabpagenr][idx]
    endif
  endif
endfunction

augroup ClapBuffers
  autocmd!
  if exists('g:clap_provider_buffers_cur_tab_only')
    autocmd BufEnter             * call s:OnBufEnter(+expand('<abuf>'))
  endif
  autocmd BufDelete            * call s:OnBufDelete(+expand('<abuf>'))
  autocmd BufWinEnter,WinEnter * let g:__clap_buffers[bufnr('')] = reltimefloat(reltime())
augroup END

" yanks provider
if get(g:, 'clap_enable_yanks_provider', 1)
  augroup ClapYanks
    autocmd!
    autocmd VimEnter * call clap#provider#yanks#init()
  augroup END
endif
