" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Path highlighting - dims directory prefix, brightens filename.

let s:save_cpo = &cpoptions
set cpoptions&vim

" Providers that display file paths and should have dimmed prefixes
let s:path_providers = ['files', 'git_files', 'recent_files', 'history', 'filer', 'grep', 'live_grep', 'proj_tags', 'tags', 'jumps']

" Check if the current provider should have path highlighting
function! clap#highlighter#path#should_highlight() abort
  if !get(g:, 'clap_enable_path_dimming', 1)
    return v:false
  endif
  return index(s:path_providers, g:clap.provider.id) >= 0
endfunction

" Find path boundaries in a line
" Returns: [prefix_start, prefix_end, filename_start, filename_end] or [] if not a path
" All indices are byte-based, 0-indexed
function! s:find_path_boundaries(line) abort
  if empty(a:line)
    return []
  endif

  " Determine the end of file path (before : for grep format, or end of line)
  let l:path_end = len(a:line) - 1

  " Check for grep format: file:line:col:content
  " Find first colon that's followed by a digit (line number)
  let l:colon_pos = 0
  while l:colon_pos < len(a:line)
    let l:colon_pos = stridx(a:line, ':', l:colon_pos)
    if l:colon_pos == -1
      break
    endif
    " Check if next char is a digit (line number)
    if l:colon_pos + 1 < len(a:line) && a:line[l:colon_pos + 1] =~# '\d'
      let l:path_end = l:colon_pos - 1
      break
    endif
    let l:colon_pos += 1
  endwhile

  " Determine where the actual path starts (skip icon if present)
  let l:path_start = 0
  if g:__clap_icon_added_by_maple
    " Icon is typically a multi-byte character + space
    " Scan for first alnum or path character after initial icon bytes
    let l:byte_idx = 0
    for l:char in split(a:line, '\zs')
      if l:byte_idx > 0 && (l:char =~# '[a-zA-Z0-9_./\\]')
        let l:path_start = l:byte_idx
        break
      endif
      let l:byte_idx += len(l:char)
    endfor
  endif

  " Find the last path separator before path_end
  let l:last_sep = -1
  let l:byte_idx = 0
  for l:char in split(a:line[:l:path_end], '\zs')
    if l:char ==# '/' || l:char ==# '\'
      let l:last_sep = l:byte_idx
    endif
    let l:byte_idx += len(l:char)
  endfor

  " Return: [prefix_start, prefix_end, filename_start, filename_end]
  " For root-level files (no separator), prefix is -1,-1 and filename covers the whole name
  if l:last_sep == -1
    return [-1, -1, l:path_start, l:path_end]
  endif

  return [l:path_start, l:last_sep, l:last_sep + 1, l:path_end]
endfunction

" Use matchaddpos for higher priority highlighting (works in display window)
" This is the same approach used by fuzzy match highlighting

" Store match IDs in window-local variable for cleanup
function! clap#highlighter#path#clear_matches() abort
  if exists('w:clap_path_match_ids')
    for l:id in w:clap_path_match_ids
      try
        call matchdelete(l:id)
      catch
        " Ignore if already deleted
      endtry
    endfor
  endif
  let w:clap_path_match_ids = []
endfunction

function! s:clear_path_highlights(bufnr) abort
  if exists('*win_execute') && exists('g:clap.display.winid')
    call win_execute(g:clap.display.winid, 'call clap#highlighter#path#clear_matches()')
  endif
endfunction

" lnum is 0-based, col_start is 0-based byte index
function! clap#highlighter#path#add_match(lnum, col_start, length, hl_group) abort
  " matchaddpos uses 1-based line and column numbers
  try
    let l:id = matchaddpos(a:hl_group, [[a:lnum + 1, a:col_start + 1, a:length]], 5)
    if !exists('w:clap_path_match_ids')
      let w:clap_path_match_ids = []
    endif
    call add(w:clap_path_match_ids, l:id)
  catch
    " Ignore errors
  endtry
endfunction

if has('nvim')
  function! s:add_path_highlight(bufnr, lnum, col_start, col_end, hl_group) abort
    let l:length = a:col_end - a:col_start + 1
    if exists('*win_execute') && exists('g:clap.display.winid')
      call win_execute(g:clap.display.winid,
            \ 'call clap#highlighter#path#add_match(' . a:lnum . ',' . a:col_start . ',' . l:length . ',"' . a:hl_group . '")')
    endif
  endfunction
else
  function! s:add_path_highlight(bufnr, lnum, col_start, col_end, hl_group) abort
    try
      call prop_add(a:lnum + 1, a:col_start + 1, {
            \ 'length': a:col_end - a:col_start + 1,
            \ 'type': a:hl_group,
            \ 'bufnr': a:bufnr
            \ })
    catch
      " Ignore errors
    endtry
  endfunction
endif

" Debug function to check if path highlighting is working
function! clap#highlighter#path#debug() abort
  echo 'Provider: ' . g:clap.provider.id
  echo 'Should highlight: ' . clap#highlighter#path#should_highlight()
  echo 'Icon added: ' . g:__clap_icon_added_by_maple
  echo 'Display bufnr: ' . g:clap.display.bufnr
  echo 'Display winid: ' . g:clap.display.winid
  echo ''

  " Test with actual display lines
  let l:lines = getbufline(g:clap.display.bufnr, 1, 5)
  echo 'Display lines (' . len(l:lines) . '):'
  for l:i in range(len(l:lines))
    echo 'Line ' . l:i . ': "' . l:lines[l:i] . '"'
    let l:bounds = s:find_path_boundaries(l:lines[l:i])
    echo '  Bounds: ' . string(l:bounds)
    if !empty(l:bounds)
      let [l:ps, l:pe, l:fs, l:fe] = l:bounds
      if l:ps >= 0
        echo '  Prefix: "' . l:lines[l:i][l:ps : l:pe] . '"'
      else
        echo '  Prefix: (none - root level file)'
      endif
      echo '  Filename: "' . l:lines[l:i][l:fs : l:fe] . '"'
    endif
  endfor

  echo ''
  echo '--- Synthetic tests ---'
  " Test with a path that has directories
  let l:test_line = ' autoload/clap/picker.vim'
  echo 'With dir: "' . l:test_line . '"'
  let l:bounds = s:find_path_boundaries(l:test_line)
  if !empty(l:bounds)
    let [l:ps, l:pe, l:fs, l:fe] = l:bounds
    if l:ps >= 0
      echo '  Prefix: "' . l:test_line[l:ps : l:pe] . '"'
    endif
    echo '  Filename: "' . l:test_line[l:fs : l:fe] . '"'
  endif

  " Test with a root-level file
  let l:test_line2 = ' Cargo.toml'
  echo 'Root file: "' . l:test_line2 . '"'
  let l:bounds2 = s:find_path_boundaries(l:test_line2)
  if !empty(l:bounds2)
    let [l:ps, l:pe, l:fs, l:fe] = l:bounds2
    if l:ps >= 0
      echo '  Prefix: "' . l:test_line2[l:ps : l:pe] . '"'
    else
      echo '  Prefix: (none)'
    endif
    echo '  Filename: "' . l:test_line2[l:fs : l:fe] . '"'
  endif

  echo ''
  echo 'ClapPathPrefix exists: ' . hlexists('ClapPathPrefix')
  echo 'ClapFileName exists: ' . hlexists('ClapFileName')

  " Check if matches are applied in the display window
  if exists('g:clap.display.winid')
    let l:match_ids = getwinvar(g:clap.display.winid, 'clap_path_match_ids', [])
    echo 'Path match IDs in display window: ' . string(l:match_ids)
  endif

  echo ''
  echo 'TIP: Type a query like "src/" or "autoload/" to see files with directory paths'
  echo 'Run :call clap#highlighter#path#apply() to manually re-apply highlights'
endfunction

" Apply path highlighting to all visible lines in the display buffer
function! clap#highlighter#path#apply() abort
  if !clap#highlighter#path#should_highlight()
    return
  endif

  let l:bufnr = g:clap.display.bufnr
  if !bufexists(l:bufnr)
    return
  endif

  " Clear previous path highlights
  call s:clear_path_highlights(l:bufnr)

  " Get all lines in the display buffer
  let l:lines = getbufline(l:bufnr, 1, '$')

  " Apply highlighting to each line
  let l:lnum = 0
  for l:line in l:lines
    if empty(l:line) || l:line ==# g:clap_no_matches_msg
      let l:lnum += 1
      continue
    endif

    let l:bounds = s:find_path_boundaries(l:line)
    if !empty(l:bounds)
      let [l:prefix_start, l:prefix_end, l:filename_start, l:filename_end] = l:bounds

      " Highlight directory prefix (dimmed) - includes the trailing /
      if l:prefix_end > l:prefix_start
        call s:add_path_highlight(l:bufnr, l:lnum, l:prefix_start, l:prefix_end, 'ClapPathPrefix')
      endif

      " Highlight filename (bright)
      if l:filename_end >= l:filename_start
        call s:add_path_highlight(l:bufnr, l:lnum, l:filename_start, l:filename_end, 'ClapFileName')
      endif
    endif

    let l:lnum += 1
  endfor
endfunction

" Clear path highlights
function! clap#highlighter#path#clear() abort
  if bufexists(g:clap.display.bufnr)
    call s:clear_path_highlights(g:clap.display.bufnr)
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
