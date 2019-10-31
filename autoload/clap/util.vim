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

function! clap#util#get_git_root() abort
  let root = split(system('git rev-parse --show-toplevel'), '\n')[0]
  return v:shell_error ? '' : root
endfunction

" This is faster than clap#util#get_git_root() which uses the system call.
function! clap#util#find_git_root(bufnr) abort
  let git_dir = clap#util#find_nearest_dir(a:bufnr, '.git')

  if !empty(git_dir)
    return fnamemodify(git_dir, ':h:h')
  endif

  return ''
endfunction

" Find the nearest directory by searching upwards
" through the paths relative to the given buffer,
" given a bufnr and a directory name.
function! clap#util#find_nearest_dir(bufnr, dir) abort
  let fname = fnameescape(fnamemodify(bufname(a:bufnr), ':p'))

  let relative_path = finddir(a:dir, fname . ';')

  if !empty(relative_path)
    return fnamemodify(relative_path, ':p')
  endif

  return ''
endfunction

" Argument: Funcref to run as well as its args
function! clap#util#run_rooter(Run, ...) abort
  if get(g:, 'clap_disable_run_rooter', v:false)
        \ || !g:clap.provider.has_enable_rooter()
        \ || getbufvar(g:clap.start.bufnr, '&bt') ==# 'terminal'
    return call(a:Run, a:000)
  endif

  let git_root = clap#util#find_git_root(g:clap.start.bufnr)

  if empty(git_root)
    let result = call(a:Run, a:000)
  else
    let save_cwd = getcwd()
    try
      execute 'lcd' git_root
      let result = call(a:Run, a:000)
    finally
      execute 'lcd' save_cwd
    endtry
  endif

  return result
endfunction

" This is used for the sink function.
"
" what if the sink function changes cwd intentionally? Then we
" should not restore to the current cwd after executing the sink function.
function! clap#util#run_rooter_heuristic(Run, ...) abort
  if getbufvar(g:clap.start.bufnr, '&bt') ==# 'terminal'
    return call(a:Run, a:000)
  endif

  let git_root = clap#util#find_git_root(g:clap.start.bufnr)

  if empty(git_root)
        \ || get(g:, 'clap_disable_run_rooter', v:false)
        \ || !g:clap.provider.has_enable_rooter()

    let result = call(a:Run, a:000)

  else

    let save_cwd = getcwd()
    try
      execute 'lcd' git_root
      let result = call(a:Run, a:000)
    finally
      " Here we could use a naive heuristic approach to
      " not restore the old cwd when the current working
      " directory is not git root or &autochdir is on.
      " This way is mainly borrowed from fzf.vim.
      if getcwd() ==# git_root && !&autochdir
        execute 'lcd' save_cwd
      endif
    endtry

  endif
endfunction

" Define CTRL-T/X/V by default.
function! clap#util#define_open_action_mappings() abort
  for k in keys(g:clap_open_action)
    let lhs = substitute(toupper(k), 'CTRL', 'C', '')
    execute 'inoremap <silent> <buffer> <'.lhs.'> <Esc>:call clap#handler#try_open("'.k.'")<CR>'
  endfor
endfunction

function! clap#util#trim_leading(str) abort
  return substitute(a:str, '^\s*', '', '')
endfunction

" Given the origin lnum and the size of range, return
" [origin_lnum-range_size, origin_lnum+range_size] and the target lnum that
" the origin line should be positioned.
" 0-based
function! clap#util#get_preview_line_range(origin_lnum, range_size) abort
  if a:origin_lnum - a:range_size > 0
    return [a:origin_lnum - a:range_size, a:origin_lnum + a:range_size, a:range_size]
  else
    return [0, a:origin_lnum + a:range_size, a:origin_lnum]
  endif
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

function! clap#util#add_match_at(lnum, col, hl_group) abort
  call matchaddpos(a:hl_group, [[a:lnum+1, a:col+1, 1]])
endfunction

if has('nvim')
  " 0-based
  function! clap#util#add_highlight_at(lnum, col, hl_group) abort
    call nvim_buf_add_highlight(g:clap.display.bufnr, -1, a:hl_group, a:lnum, a:col, a:col+1)
  endfunction
else
  function! clap#util#add_highlight_at(lnum, col, hl_group) abort
    " 1-based
    call prop_add(a:lnum+1, a:col+1, {'length': 1, 'type': a:hl_group, 'bufnr': g:clap.display.bufnr})
  endfunction
endif

let &cpoptions = s:save_cpo
unlet s:save_cpo
