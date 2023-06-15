" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Utilities for sink function.

let s:save_cpo = &cpoptions
set cpoptions&vim

function! clap#sink#edit_with_open_action(fpath) abort
  if has_key(g:clap, 'open_action')
    execute g:clap.open_action a:fpath
  else
    " Cannot use noautocmd here as it would lose syntax, and ...
    execute 'edit' fnameescape(a:fpath)
  endif
endfunction

function! clap#sink#open_file(fpath, lnum, col) abort
  normal! m'
  call clap#sink#edit_with_open_action(a:fpath)
  noautocmd call cursor(a:lnum, a:col)
  normal! zz
endfunction

function! clap#sink#open_quickfix(qf_entries) abort
  let entries_len = len(a:qf_entries)
  let w:quickfix_title = 'Search'
  call setqflist([], ' ', {'title': 'Search', 'items': a:qf_entries })
  " If there are only a few items, open the qf window at exact size.
  if entries_len < 15
    execute 'copen' entries_len
  else
    copen
  endif
  cc
endfunction

function! clap#sink#open_results(entries) abort
  let ordered_entries = {}
  for entry in a:entries
    if has_key(ordered_entries, entry.filename)
      call add(ordered_entries[entry.filename], entry)
    else
      let ordered_entries[entry.filename] = [entry]
    endif
  endfor

  function! s:SortEntries(x, y) abort
    return a:x.lnum > a:y.lnum
  endfunction

  for entries in values(ordered_entries)
    call sort(entries, function('s:SortEntries'))
  endfor

  let grep_style = v:true

  let lines = []

  if grep_style
    call add(lines, '')
    for entries in values(ordered_entries)
      for entry in entries
        call add(lines, printf('%s %s:%d:%d:%s', clap#icon#get(entry.filename), entry.filename, entry.lnum, entry.col, entry.text))
      endfor
    endfor
  else
    for entries in values(ordered_entries)
      call add(lines, '')
      call add(lines, printf('%s %s [%d]', clap#icon#get(entry.filename), entry.filename, len(entries)))
      for entry in entries
        call add(lines, printf('    %d:%d:%s', entry.lnum, entry.col, entry.text))
      endfor
    endfor
  endif

  let s:prev_winid = g:clap.start.winid
  echom 's:prev_winid:'.s:prev_winid

  if &columns > 200
    vnew
  else
    botright new
    execute 'resize' len(a:entries)
  endif

  function! OnCursorMoved() abort
    let lnum = line('.')
    if lnum < 3
      return
    endif
    let line = getline('.')
    if empty(line)
      return
    endif
    let [fpath, lnum, col] = clap#provider#live_grep#parse_line(line)
    call win_gotoid(s:prev_winid)
    execute 'edit' fpath
    call cursor(lnum, col)
    normal! zz
    noautocmd wincmd p
  endfunction

  function! NextFile() abort
  endfunction

  setlocal buftype=nofile nobuflisted bufhidden=wipe noswapfile syntax=clap_grep nonumber norelativenumber signcolumn=no

  nnoremap <silent> <buffer> ]f   :<c-u>call NextFile()<CR>
  nnoremap <silent> <buffer> [f   :<c-u>call PrevFile()<CR>

  autocmd CursorMoved <buffer> call OnCursorMoved()

  call append(line('$'), lines)

  let total_files = len(ordered_entries)
  call setbufline('', 1, printf("=== Found %d results in %d files ===", len(a:entries), total_files))

  syntax match qfTitle "===\zs[^=]*\ze==="
  syntax match qfTitleNumber " \zs\d\+\ze "

  highlight default link qfTitle Title
  highlight default link qfTitleNumber Number
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
