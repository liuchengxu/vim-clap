" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Utilities.

let s:save_cpo = &cpo
set cpo&vim

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

function! clap#util#run_from_project_root(Run) abort
  let git_root = clap#util#find_git_root(g:clap.start.bufnr)
  if empty(git_root) || get(g:, 'clap_disable_run_from_project_root', v:false)
    let result = a:Run()
  else
    let save_cwd = getcwd()
    try
      execute 'lcd' git_root
      let result = a:Run()
    finally
      execute 'lcd' save_cwd
    endtry
  endif
  return result
endfunction

" TODO: expandcmd() 8.1.1510 https://github.com/vim/vim/commit/80dad48
function! clap#util#expand(args) abort
  if a:args == ['<cword>']
    return [expand('<cword>')]
  endif
  return a:args
endfunction

let &cpo = s:save_cpo
unlet s:save_cpo
