" vim-clap - Modern interactive filter and dispatcher
" Author:    Liu-Cheng Xu <xuliuchengxlc@gmail.com>
" Website:   https://github.com/liuchengxu/vim-clap
" Version:   0.54
" License:   MIT

if exists('g:loaded_clap')
  finish
endif

let g:loaded_clap = 1

if get(g:, 'clap_start_server_on_startup', 1)
  call clap#job#daemon#start()
endif

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

augroup VimClap
  autocmd!

  if exists('g:clap_provider_buffers_cur_tab_only')
    autocmd BufEnter           * call s:OnBufEnter(+expand('<abuf>'))
  endif
  autocmd BufDelete            * call s:OnBufDelete(+expand('<abuf>'))
  autocmd BufWinEnter,WinEnter * let g:__clap_buffers[bufnr('')] = reltimefloat(reltime())

  autocmd BufAdd      * call clap#client#notify('__noteRecentFiles', [+expand('<abuf>')])

  if get(g:, 'clap_plugin_experimental', 0)
    autocmd InsertEnter  * call clap#client#notify('InsertEnter',  [+expand('<abuf>')])
    autocmd CursorMoved  * call clap#client#notify('CursorMoved',  [+expand('<abuf>')])
    autocmd BufNewFile   * call clap#client#notify('BufNewFile',   [+expand('<abuf>')])
    autocmd BufEnter     * call clap#client#notify('BufEnter',     [+expand('<abuf>')])
    autocmd BufLeave     * call clap#client#notify('BufLeave',     [+expand('<abuf>')])
    autocmd BufDelete    * call clap#client#notify('BufDelete',    [+expand('<abuf>')])
    autocmd BufWritePost * call clap#client#notify('BufWritePost', [+expand('<abuf>')])
    autocmd BufWinEnter  * call clap#client#notify('BufWinEnter',  [+expand('<abuf>')])
    autocmd BufWinLeave  * call clap#client#notify('BufWinLeave',  [+expand('<abuf>')])
    " Are these really needed?
    " autocmd TextChanged  * call clap#client#notify('TextChanged',  [+expand('<abuf>')])
    autocmd TextChangedI * call clap#client#notify('TextChangedI', [+expand('<abuf>')])

    " Create `clap_actions` provider so that it's convenient to interact with the plugins later.
    let g:clap_provider_clap_actions = get(g:, 'clap_provider_clap_actions', {
                \ 'source': { -> get(g:, 'clap_actions', []) },
                \ 'sink': { line -> clap#client#notify(line, [ g:clap.start.bufnr, g:clap.start.old_pos[1], g:clap.start.old_pos[2] ] ) },
                \ 'mode': 'quick_pick',
                \ })

    function! s:RequestClapAction(bang, action) abort
      call clap#client#notify(a:action, [])
    endfunction

    command! -bang -nargs=* -bar -range -complete=customlist,clap#helper#complete_actions ClapAction call s:RequestClapAction(<bang>0, <f-args>)
  endif

  " yanks provider
  if get(g:, 'clap_enable_yanks_provider', 1)
    autocmd VimEnter * call clap#provider#yanks#init()
  endif
augroup END
