" Author: liuchengxu <xuliuchengxlc@gmail.com>

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

highlight default link ClapBlameInfo SpecialComment

let s:sign_group_added = 'ClapGitAdded'
let s:sign_group_modified = 'ClapGitModified'
let s:sign_group_removed = 'ClapGitRemoved'
let s:sign_group_modified_removed = 'ClapGitModifiedRemoved'
let s:sign_group_removed_above_and_below = 'ClapGitRemovedAboveAndBelow'

call sign_define(s:sign_group_added, get(g:, 'clap_sign_added', {
      \ 'text': '+',
      \ 'texthl': 'DiffAdd',
      \ }))
call sign_define(s:sign_group_modified, get(g:, 'clap_sign_modified', {
      \ 'text': '~',
      \ 'texthl': 'DiffChange',
      \ }))
call sign_define(s:sign_group_removed, get(g:, 'clap_sign_removed', {
      \ 'text': '_',
      \ 'texthl': 'DiffDelete',
      \ }))
call sign_define(s:sign_group_modified_removed, get(g:, 'clap_sign_modified_removed', {
      \ 'text': '~_',
      \ 'texthl': 'DiffDelete',
      \ }))
call sign_define(s:sign_group_removed_above_and_below, get(g:, 'clap_sign_removed_above_and_below', {
      \ 'text': '_-',
      \ 'texthl': 'DiffDelete',
      \ }))

function! s:place_sign_at(group, bufnr, lnum) abort
  call sign_place(0, a:group, a:group, a:bufnr, {'lnum': a:lnum})
endfunction

function! s:unplace_sign_at(group, bufnr) abort
  call sign_unplace(a:group, { 'buffer': a:bufnr })
endfunction

function! s:process_signs_added(bufnr, added) abort
  let start = a:added.start
  let end = a:added.end
  for lnum in range(start, end-1)
    call s:place_sign_at(s:sign_group_added, a:bufnr, lnum)
  endfor
endfunction

function! s:process_signs_modified(bufnr, modified) abort
  let start = a:modified.start
  let end = a:modified.end
  for lnum in range(start, end-1)
    call s:place_sign_at(s:sign_group_modified, a:bufnr, lnum)
  endfor
endfunction

function! s:process_signs_removed(bufnr, lnum) abort
  call s:place_sign_at(s:sign_group_removed, a:bufnr, a:lnum)
endfunction

function! clap#plugin#git#add_diff_signs(bufnr, modifications) abort
  echom string(a:modifications)
  for modification in a:modifications
    " RemovedFirstLine
    if type(modification) == v:t_string
      call s:process_signs_removed(a:bufnr, 1)
    elseif has_key(modification, 'Added')
      call s:process_signs_added(a:bufnr, modification.Added)
    elseif has_key(modification, 'Modified')
      call s:process_signs_modified(a:bufnr, modification.Modified)
    elseif has_key(modification, 'Removed')
      call s:process_signs_removed(a:bufnr, modification.Removed)
    elseif has_key(modification, 'ModifiedAndAdded')
      call s:process_signs_modified(a:bufnr, modification.ModifiedAndAdded.modified)
      call s:process_signs_added(a:bufnr, modification.ModifiedAndAdded.added)
    elseif has_key(modification, 'ModifiedAndRemoved')
      call s:process_signs_modified(a:bufnr, modification.ModifiedAndRemoved.modified)
      call s:place_sign_at(s:sign_group_modified_removed, a:bufnr, modification.ModifiedAndRemoved.modified_removed)
    elseif has_key(modification, 'RemovedAboveAndBelow')
      call s:place_sign_at(s:sign_group_removed_above_and_below, a:bufnr, 1)
    endif
  endfor
endfunction

function! clap#plugin#git#clear_diff_signs(bufnr) abort
  call sign_unplace(s:sign_group_added, {'buffer': a:bufnr })
  call sign_unplace(s:sign_group_modified, {'buffer': a:bufnr })
  call sign_unplace(s:sign_group_removed, {'buffer': a:bufnr })
endfunction

if has('nvim')
  function! clap#plugin#git#clear_blame_info(bufnr) abort
    let id = getbufvar(a:bufnr, 'clap_git_blame_extmark_id')
    if !empty(id)
      call nvim_buf_del_extmark(a:bufnr, s:blame_ns_id, id)
    endif
  endfunction

  function! clap#plugin#git#show_cursor_blame_info(bufnr, text) abort
    if !exists('s:blame_ns_id')
      let s:blame_ns_id = nvim_create_namespace('clap_blame')
    endif

    let id = getbufvar(a:bufnr, 'clap_git_blame_extmark_id')
    if !empty(id)
      call nvim_buf_del_extmark(a:bufnr, s:blame_ns_id, id)
    endif

    let always_eol = v:true
    let available_space = winwidth(bufwinid(a:bufnr)) - col('$')
    if always_eol || available_space > strlen(a:text)
      let opts = { 'virt_text': [[a:text, 'ClapBlameInfo']], 'virt_text_pos': 'eol' }
    else
      let text = &signcolumn ==# 'yes' ? printf('  ╰─▸ %s', a:text) : a:text
      let text = &numberwidth > 0 ? printf('%s%s', repeat(' ', &numberwidth/2), text) : text
      let opts = { 'virt_lines': [[[text, 'ClapBlameInfo']]], 'virt_lines_leftcol': col('.')  - 1 }
    endif

    try
      let last_id = nvim_buf_set_extmark(a:bufnr, s:blame_ns_id, line('.') - 1, col('.') - 1, opts)
      call setbufvar(a:bufnr, 'clap_git_blame_extmark_id', last_id)
    " Suppress error: Invalid 'col': out of range
    catch /^Vim\%((\a\+)\)\=:E5555/
    endtry
  endfunction

else

  function! clap#plugin#git#clear_blame_info(bufnr) abort
    " Popup will be closed automatically due to the `moved` option.
  endfunction

  function! clap#plugin#git#show_cursor_blame_info(bufnr, text) abort
    let col_offset = 4 + col('$') - col('.')
    let popup_id = popup_create(a:text, {
          \ 'line': 'cursor',
          \ 'col': printf('cursor+%d', col_offset),
          \ 'highlight': 'ClapBlameInfo',
          \ 'wrap': v:true,
          \ 'zindex': 100,
          \ 'moved': [line('.'), 1, -1],
          \ })
  endfunction

endif

let &cpoptions = s:save_cpo
unlet s:save_cpo
