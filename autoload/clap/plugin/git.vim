" Author: liuchengxu <xuliuchengxlc@gmail.com>

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

highlight ClapSignAdd    ctermfg=70  guifg=#67b11d
highlight ClapSignDelete ctermfg=196 guifg=#f2241f
highlight ClapSignChange ctermfg=173 guifg=#e18254

highlight default link ClapGitSignAdd    ClapSignAdd
highlight default link ClapGitSignDelete ClapSignDelete
highlight default link ClapGitSignChange ClapSignChange

highlight default link ClapBlameInfo SpecialComment

let s:git_signs = {
      \ 'added': { 'text': '+', 'texthl': 'ClapGitSignAdd' },
      \ 'modified': { 'text': '~', 'texthl': 'ClapGitSignChange' },
      \ 'removed': { 'text': '_', 'texthl': 'ClapGitSignDelete' },
      \ 'modified_removed': { 'text': '~_', 'texthl': 'ClapGitSignDelete' },
      \ 'removed_above_and_below': { 'text': '_-', 'texthl': 'ClapGitSignDelete' },
      \ }

if exists('g:clap_plugin_git_signs')
  call extend(s:git_signs, g:clap_plugin_git_signs)
endif

let s:sign_name_added = 'ClapGitAdded'
let s:sign_name_modified = 'ClapGitModified'
let s:sign_name_removed = 'ClapGitRemoved'
let s:sign_name_modified_removed = 'ClapGitModifiedRemoved'
let s:sign_name_removed_above_and_below = 'ClapGitRemovedAboveAndBelow'

call sign_define(s:sign_name_added, s:git_signs.added)
call sign_define(s:sign_name_modified, s:git_signs.modified)
call sign_define(s:sign_name_removed, s:git_signs.removed)
call sign_define(s:sign_name_modified_removed, s:git_signs.modified_removed)
call sign_define(s:sign_name_removed_above_and_below, s:git_signs.removed_above_and_below)

function! s:place_sign_at(group, bufnr, lnum) abort
  call sign_place(0, 'clap_git_buffer_signs', a:group, a:bufnr, {'lnum': a:lnum})
endfunction

function! s:process_signs_added(bufnr, added) abort
  let start = a:added.start
  let end = a:added.end
  call map(range(start, end-1), 's:place_sign_at(s:sign_name_added, a:bufnr, v:val)')
endfunction

function! s:process_signs_modified(bufnr, modified) abort
  let start = a:modified.start
  let end = a:modified.end
  call map(range(start, end-1), 's:place_sign_at(s:sign_name_modified, a:bufnr, v:val)')
endfunction

function! s:process_signs_removed(bufnr, lnum) abort
  call s:place_sign_at(s:sign_name_removed, a:bufnr, a:lnum)
endfunction

function! clap#plugin#git#add_diff_signs(bufnr, modifications) abort
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
      call s:place_sign_at(s:sign_name_modified_removed, a:bufnr, modification.ModifiedAndRemoved.modified_removed)
    elseif has_key(modification, 'RemovedAboveAndBelow')
      call s:place_sign_at(s:sign_name_removed_above_and_below, a:bufnr, 1)
    endif
  endfor
endfunction

function! clap#plugin#git#clear_diff_signs(bufnr) abort
  call sign_unplace('clap_git_buffer_signs', {'buffer': a:bufnr })
endfunction

let s:visual_signs_group = 'clap_git_visual_signs'

function! clap#plugin#git#refresh_visual_signs(bufnr, signs) abort
  call sign_unplace(s:visual_signs_group, {'buffer': a:bufnr })
  call clap#plugin#git#add_visual_signs(a:bufnr, a:signs)
endfunction

function! clap#plugin#git#add_visual_signs(bufnr, signs) abort
  for [lnum, sign_type] in a:signs
    if sign_type ==# 'A'
      call sign_place(lnum, s:visual_signs_group, s:sign_name_added, a:bufnr, {'lnum': lnum})
    elseif sign_type ==# 'M'
      call sign_place(lnum, s:visual_signs_group, s:sign_name_modified, a:bufnr, {'lnum': lnum})
    elseif sign_type ==# 'R'
      call sign_place(lnum, s:visual_signs_group, s:sign_name_removed, a:bufnr, {'lnum': lnum})
    elseif sign_type ==# 'MR'
      call sign_place(lnum, s:visual_signs_group, s:sign_name_modified_removed, a:bufnr, {'lnum': lnum})
    elseif sign_type ==# 'RA'
      call sign_place(lnum, s:visual_signs_group, s:sign_name_removed_above_and_below, a:bufnr, {'lnum': lnum})
    endif
  endfor
endfunction

function! clap#plugin#git#clear_visual_signs(bufnr) abort
  call sign_unplace(s:visual_signs_group, {'buffer': a:bufnr })
endfunction

function! clap#plugin#git#set_summary_var(bufnr, summary) abort
  let clap_git = getbufvar(a:bufnr, 'clap_git', {})
  let clap_git.summary = a:summary
  call setbufvar(a:bufnr, 'clap_git', clap_git)
endfunction

function! clap#plugin#git#set_branch_var(bufnr, branch) abort
  let clap_git = getbufvar(a:bufnr, 'clap_git', {})
  let clap_git.branch = a:branch
  call setbufvar(a:bufnr, 'clap_git', clap_git)
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
