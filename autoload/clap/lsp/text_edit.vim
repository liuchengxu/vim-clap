function! clap#lsp#text_edit#get_range(text_edit) abort
  if type(a:text_edit) != v:t_dict
    return v:null
  endif
  let l:insert = get(a:text_edit, 'insert', v:null)
  if type(l:insert) == v:t_dict
    return l:insert
  endif
  return get(a:text_edit, 'range', v:null)
endfunction

" The inverse version of `s:to_col`.
" Convert [lnum, col] to LSP's `Position`.
function! s:to_char(expr, lnum, col) abort
    let l:lines = getbufline(a:expr, a:lnum)
    if l:lines == []
        if type(a:expr) != v:t_string || !filereadable(a:expr)
            " invalid a:expr
            return a:col - 1
        endif
        " a:expr is a file that is not yet loaded as a buffer
        let l:lines = readfile(a:expr, '', a:lnum)
    endif
    let l:linestr = l:lines[-1]
    return strchars(strpart(l:linestr, 0, a:col - 1))
endfunction

function! s:get_position(...) abort
    let l:line = line('.')
    let l:char = s:to_char('%', l:line, col('.'))
    return { 'line': l:line - 1, 'character': l:char }
endfunction

function! clap#lsp#text_edit#apply_text_edits(filepath, text_edits) abort
    let l:current_bufname = bufname('%')
    let l:target_bufname = a:filepath
    let l:cursor_position = s:get_position()

    call s:_switch(l:target_bufname)
    " for l:text_edit in s:_normalize(a:text_edits)
    for l:text_edit in a:text_edits
        call s:_apply(bufnr(l:target_bufname), l:text_edit, l:cursor_position)
    endfor
    call s:_switch(l:current_bufname)

    if bufnr(l:current_bufname) == bufnr(l:target_bufname)
        call cursor(s:lsp_to_vim('%', l:cursor_position))
    endif
endfunction

function! s:decode_uri(uri) abort
    let l:ret = substitute(a:uri, '[?#].*', '', '')
    return substitute(l:ret, '%\(\x\x\)', '\=printf("%c", str2nr(submatch(1), 16))', 'g')
endfunction

" This function can be error prone if the caller forgets to use +1 to vim line
" so use lsp#utils#position#lsp_to_vim instead
" Convert a character-index (0-based) to byte-index (1-based)
" This function requires a buffer specifier (expr, see :help bufname()),
" a line number (lnum, 1-based), and a character-index (char, 0-based).
function! s:to_col(expr, lnum, char) abort
    let l:lines = getbufline(a:expr, a:lnum)
    if l:lines == []
        if type(a:expr) != v:t_string || !filereadable(a:expr)
            " invalid a:expr
            return a:char + 1
        endif
        " a:expr is a file that is not yet loaded as a buffer
        let l:lines = readfile(a:expr, '', a:lnum)
        if l:lines == []
            " when the file is empty. a:char should be 0 in the case
            return a:char + 1
        endif
    endif
    let l:linestr = l:lines[-1]
    return strlen(strcharpart(l:linestr, 0, a:char)) + 1
endfunction

function! s:is_file_uri(uri) abort
    return stridx(a:uri, 'file:///') == 0
endfunction

function! s:lsp_line_to_vim(expr, position) abort
    return a:position['line'] + 1
endfunction

function! s:lsp_character_to_vim(expr, position) abort
    let l:line = a:position['line'] + 1 " optimize function overhead by not calling lsp_line_to_vim
    let l:char = a:position['character']
    return s:to_col(a:expr, l:line, l:char)
endfunction

function! s:lsp_to_vim(expr, position) abort
    let l:line = s:lsp_line_to_vim(a:expr, a:position)
    let l:col = s:lsp_character_to_vim(a:expr, a:position)
    return [l:line, l:col]
endfunction

let s:fixendofline_exists = exists('+fixendofline')

function! s:get_fixendofline(buf) abort
    let l:eol = getbufvar(a:buf, '&endofline')
    let l:binary = getbufvar(a:buf, '&binary')

    if s:fixendofline_exists
        let l:fixeol = getbufvar(a:buf, '&fixendofline')

        if !l:binary
            " When 'binary' is off and 'fixeol' is on, 'endofline' is not used
            "
            " When 'binary' is off and 'fixeol' is off, 'endofline' is used to
            " remember the presence of a <EOL>
            return l:fixeol || l:eol
        else
            " When 'binary' is on, the value of 'fixeol' doesn't matter
            return l:eol
        endif
    else
        " When 'binary' is off the value of 'endofline' is not used
        "
        " When 'binary' is on 'endofline' is used to remember the presence of
        " a <EOL>
        return !l:binary || l:eol
    endif
endfunction

function! s:_split_by_eol(text) abort
    return split(a:text, '\r\n\|\r\|\n', v:true)
endfunction

"
" _apply
"
function! s:_apply(bufnr, text_edit, cursor_position) abort
    " create before/after line.
    let l:start_line = getline(a:text_edit['range']['start']['line'] + 1)
    let l:end_line = getline(a:text_edit['range']['end']['line'] + 1)
    let l:before_line = strcharpart(l:start_line, 0, a:text_edit['range']['start']['character'])
    let l:after_line = strcharpart(l:end_line, a:text_edit['range']['end']['character'], strchars(l:end_line) - a:text_edit['range']['end']['character'])

    " create new lines.
    let l:new_lines = s:_split_by_eol(a:text_edit['newText'])
    let l:new_lines[0] = l:before_line . l:new_lines[0]
    let l:new_lines[-1] = l:new_lines[-1] . l:after_line

  " save length.
    let l:new_lines_len = len(l:new_lines)
    let l:range_len = (a:text_edit['range']['end']['line'] - a:text_edit['range']['start']['line']) + 1

    " fixendofline
    let l:buffer_length = len(getbufline(a:bufnr, '^', '$'))
    let l:should_fixendofline = s:get_fixendofline(a:bufnr)
    let l:should_fixendofline = l:should_fixendofline && l:new_lines[-1] ==# ''
    let l:should_fixendofline = l:should_fixendofline && l:buffer_length <= a:text_edit['range']['end']['line']
    let l:should_fixendofline = l:should_fixendofline && a:text_edit['range']['end']['character'] == 0
    if l:should_fixendofline
        call remove(l:new_lines, -1)
    endif

    " fix cursor pos
    if a:text_edit['range']['end']['line'] < a:cursor_position['line']
        " fix cursor line
        let a:cursor_position['line'] += l:new_lines_len - l:range_len
    elseif a:text_edit['range']['end']['line'] == a:cursor_position['line'] && a:text_edit['range']['end']['character'] <= a:cursor_position['character']
        " fix cursor line and col
        let a:cursor_position['line'] += l:new_lines_len - l:range_len
        let l:end_character = strchars(l:new_lines[-1]) - strchars(l:after_line)
        let l:end_offset = a:cursor_position['character'] - a:text_edit['range']['end']['character']
        let a:cursor_position['character'] = l:end_character + l:end_offset
    endif

    " append or delete lines.
    if l:new_lines_len > l:range_len
        call append(a:text_edit['range']['start']['line'], repeat([''], l:new_lines_len - l:range_len))
    elseif l:new_lines_len < l:range_len
        let l:offset = l:range_len - l:new_lines_len
        call s:delete(a:bufnr, a:text_edit['range']['start']['line'] + 1, a:text_edit['range']['start']['line'] + l:offset)
    endif

    " set lines.
    call setline(a:text_edit['range']['start']['line'] + 1, l:new_lines)
endfunction

"
" _check
"
" LSP Spec says `multiple text edits can not overlap those ranges`.
" This function check it. But does not throw error.
"
function! s:_check(text_edits) abort
  if len(a:text_edits) > 1
    let l:range = a:text_edits[0].range
    for l:text_edit in a:text_edits[1 : -1]
      if l:range.end.line > l:text_edit.range.start.line || (
      \   l:range.end.line == l:text_edit.range.start.line &&
      \   l:range.end.character > l:text_edit.range.start.character
      \ )
        echom 'ERROR: text_edit: range overlapped.'
      endif
      let l:range = l:text_edit.range
    endfor
  endif
  return a:text_edits
endfunction

"
" _switch
"
function! s:_switch(path) abort
  if bufnr(a:path) >= 0
    execute printf('keepalt keepjumps %sbuffer!', bufnr(a:path))
  else
    execute printf('keepalt keepjumps edit! %s', fnameescape(a:path))
  endif
endfunction

"
" delete
"
function! s:delete(bufnr, start, end) abort
  if exists('*deletebufline')
      call deletebufline(a:bufnr, a:start, a:end)
  else
      let l:foldenable = &foldenable
      setlocal nofoldenable
      execute printf('%s,%sdelete _', a:start, a:end)
      let &foldenable = l:foldenable
  endif
endfunction
