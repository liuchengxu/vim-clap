" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Action dialog based on floating win.
scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

function! clap#floating_win#action#create() abort
  let buf = nvim_create_buf(v:false, v:true)

  let provider_action = g:clap.provider._().action
  if has_key(provider_action, 'title')
    let title = provider_action['title']()
  else
    let title = 'Choose action:'
  endif
  let choices = filter(keys(provider_action), 'v:val !~# "title"')
  let lines = []

  let s:lnum2key = {}
  let idx = 2
  let max_line_len = strlen(title)
  for choice in choices
    let s:lnum2key[idx] = choice
    let str = substitute(choice, '&', '', '')
    let line = join(split(str, '\ze\u'), ' ')
    if strlen(line) > max_line_len
      let max_line_len = strlen(line)
    endif
    call add(lines, line)
    let idx += 1
  endfor

  let choices = map(lines, '" [". (str2nr(v:key) + 1)."] ".v:val')

  let lines = [title] + choices
  call nvim_buf_set_lines(buf, 0, -1, v:true, lines)

  call setbufvar(buf,  '&filetype', 'clap_action')

  let display_opts = nvim_win_get_config(g:clap.display.winid)
  let opts = {}
  let opts.row = display_opts.row + display_opts.height / 3
  let opts.col = display_opts.col + display_opts.width / 5
  let opts.style = 'minimal'
  let opts.relative = 'editor'
  let opts.height = len(lines)
  let opts.width = max([display_opts.width * 3 / 5, max_line_len + 5])
  silent let s:action_winid = nvim_open_win(buf, v:true, opts)

  noautocmd call win_gotoid(s:action_winid)
  call cursor(2, 6)

  let w:action_header_id = matchaddpos('Title', [1])
endfunction

function! clap#floating_win#action#close() abort
  call clap#util#nvim_win_close_safe(s:action_winid)
  noautocmd call win_gotoid(g:clap.input.winid)
endfunction

function! clap#floating_win#action#apply_choice() abort
  if has_key(s:lnum2key, line('.'))
    let provider_action = g:clap.provider._().action
    let action_key = s:lnum2key[line('.')]
    call clap#util#nvim_win_close_safe(s:action_winid)
    " TODO: add `action*` for performing actions against multi-selected entries?
    call clap#floating_win#action#close()
    call provider_action[action_key]()
  else
    call clap#helper#echo_error('Invalid action choice: '.getline('.'))
  endif
endfunction

function! clap#floating_win#action#next_item() abort
  if line('.') == line('$')
    noautocmd call cursor(2, 6)
  else
    noautocmd call cursor(line('.') + 1, 6)
  endif
endfunction

function! clap#floating_win#action#prev_item() abort
  if line('.') == 2
    noautocmd call cursor(line('$'), 6)
  else
    noautocmd call cursor(line('.') - 1, 6)
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
