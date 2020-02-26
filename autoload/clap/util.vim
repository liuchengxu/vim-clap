" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Utilities.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:blink = {}

function! s:blink.tick(_) dict abort
  let self.ticks -= 1
  let active = self == s:blink && self.ticks > 0

  if !self.clear() && active && &hlsearch
    let w:clap_blink_id = matchaddpos('IncSearch', [s:hi_pos])
  endif
  if active
    call timer_start(self.delay, self.tick)
  endif
endfunction

function! s:blink.clear() dict abort
  if exists('w:clap_blink_id')
    call matchdelete(w:clap_blink_id)
    unlet w:clap_blink_id
    return 1
  endif
endfunction

" Try to load the file into a buffer given the file path.
" This could be used for the preview purpose.
function! clap#util#try_load_file(file) abort
  if filereadable(expand(a:file))
    let bufnr = bufadd(a:file)
    if !bufloaded(bufnr)
      " Use noautocmd here as we actually only want to get the buffer text,
      " otherwise some services may be started unexpected, e.g., LSP service.
      noautocmd silent call bufload(bufnr)
    endif
    return bufnr
  else
    return v:null
  endif
endfunction

" Blink current line under cursor, originally from junegunn/vim-slash.
function! clap#util#blink(times, delay, ...) abort
  let s:blink.ticks = 2 * a:times
  let s:blink.delay = a:delay

  let s:hi_pos = get(a:000, 0, line('.'))

  if !exists('#ClapBlink')
    augroup ClapBlink
      autocmd!
      autocmd BufWinEnter * call s:blink.clear()
    augroup END
  endif

  call s:blink.clear()
  call s:blink.tick(0)
  return ''
endfunction

function! clap#util#nvim_buf_get_first_line(bufnr) abort
  return get(nvim_buf_get_lines(a:bufnr, 0, 1, 0), 0, '')
endfunction

function! clap#util#nvim_buf_get_lines(bufnr) abort
  return nvim_buf_get_lines(a:bufnr, 0, -1, 0)
endfunction

function! clap#util#nvim_buf_set_lines(bufnr, lines) abort
  call nvim_buf_set_lines(a:bufnr, 0, -1, 0, a:lines)
endfunction

function! clap#util#nvim_buf_clear(bufnr) abort
  call nvim_buf_set_lines(a:bufnr, 0, -1, 0, [])
endfunction

function! clap#util#nvim_buf_is_empty(bufnr) abort
  let last_lnum = nvim_buf_line_count(a:bufnr)
  return last_lnum == 1 && empty(getbufline(a:bufnr, 1)[0])
endfunction

function! clap#util#nvim_buf_append_lines(bufnr, lines) abort
  if clap#util#nvim_buf_is_empty(a:bufnr)
    call nvim_buf_set_lines(a:bufnr, 0, -1, v:true, a:lines)
  else
    call nvim_buf_set_lines(a:bufnr, -1, -1, v:true, a:lines)
  endif
endfunction

function! clap#util#nvim_win_close_safe(winid) abort
  if nvim_win_is_valid(a:winid)
    call nvim_win_close(a:winid, v:true)
  endif
endfunction

" Define CTRL-T/X/V by default.
function! clap#util#define_open_action_mappings() abort
  for k in keys(g:clap_open_action)
    let lhs = substitute(toupper(k), 'CTRL', 'C', '')
    execute 'inoremap <silent> <buffer> <nowait> <'.lhs.'> <Esc>:call clap#handler#try_open("'.k.'")<CR>'
    if !g:clap_insert_mode_only
      execute 'nnoremap <silent> <buffer> <nowait> <'.lhs.'> :<c-u>call clap#handler#try_open("'.k.'")<CR>'
    endif
  endfor
endfunction

function! clap#util#trim_leading(str) abort
  return substitute(a:str, '^\s*', '', '')
endfunction

function! clap#util#buflisted() abort
  return filter(range(1, bufnr('$')), 'buflisted(v:val) && getbufvar(v:val, "&filetype") !=# "qf"')
endfunction

" Borrowed from fzf.vim
function! s:sort_buffers(...) abort
  let [b1, b2] = map(copy(a:000), 'get(g:__clap_buffers, v:val, v:val)')
  " Using minus between a float and a number in a sort function causes an error
  return b1 < b2 ? 1 : -1
endfunction

function! clap#util#buflisted_sorted() abort
  return sort(clap#util#buflisted(), 's:sort_buffers')
endfunction

" TODO: expandcmd() 8.1.1510 https://github.com/vim/vim/commit/80dad48
function! clap#util#expand(args) abort
  if a:args ==# '<cword>'
    return expand('<cword>')
  endif
  return a:args
endfunction

function! clap#util#getfsize(fname) abort
  let l:size = getfsize(expand(a:fname))
  if l:size == 0 || l:size == -1 || l:size == -2
    return ''
  endif
  if l:size < 1024
    let size = l:size.'B'
  elseif l:size < 1024*1024
    let size = printf('%.1f', l:size/1024.0) . 'K'
  elseif l:size < 1024*1024*1024
    let size = printf('%.1f', l:size/1024.0/1024.0) . 'M'
  else
    let size = printf('%.1f', l:size/1024.0/1024.0/1024.0) . 'G'
  endif
  return size
endfunction

function! clap#util#open_quickfix(qf_entries) abort
  let entries_len = len(a:qf_entries)
  call setqflist(a:qf_entries)
  " If there are only a few items, open the qf window at exact size.
  if entries_len < 15
    execute 'copen' entries_len
  else
    copen
  endif
  cc
endfunction

function! clap#util#get_visual_selection() abort
  try
    let a_save = @a
    silent normal! gv"ay
    return escape(@a, '"')
  finally
    let @a = a_save
  endtry
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
