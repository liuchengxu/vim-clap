" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Initialize and make a compatible layer between neovim and vim.

let s:save_cpo = &cpo
set cpo&vim

let s:is_nvim = has('nvim')
let s:default_priority = 10
let s:input_default_hi_group = 'Visual'
let s:display_default_hi_group = 'Pmenu'
let s:preview_defaualt_hi_group = 'PmenuSel'

function! s:_goto_win() dict abort
  noautocmd call win_gotoid(self.winid)
endfunction

function! s:_getbufvar(varname) dict abort
  return getbufvar(self.bufnr, a:varname)
endfunction

function! s:_setbufvar(varname, val) dict abort
  call setbufvar(self.bufnr, a:varname, a:val)
endfunction

function! s:_setbufvar_batch(dict) dict abort
  call map(a:dict, { key, val -> setbufvar(self.bufnr, key, val) })
endfunction

if s:is_nvim
  function! s:_get_lines() dict abort
    return nvim_buf_get_lines(self.bufnr, 0, -1, 0)
  endfunction
else
  function! s:_get_lines() dict abort
    let lines = getbufline(self.bufnr, 0, '$')
    return len(lines) == 1 && empty(lines[0]) ? [] : lines
  endfunction
endif

function! s:matchadd(patterns) abort
  let w:clap_match_ids = []
  call add(w:clap_match_ids, matchadd("ClapMatches", a:patterns[0], s:default_priority))
  let idx = 1
  " As most 8 submatches
  for p in a:patterns[1:8]
    try
      call add(w:clap_match_ids, matchadd("ClapMatches".idx, p, s:default_priority - 1))
      let idx += 1
    catch
      call clap#error(v:exception)
    endtry
  endfor
endfunction

function! s:init_display() abort
  let display = {}
  let display.goto_win = function('s:_goto_win')
  let display.get_lines = function('s:_get_lines')
  let display.getbufvar = function('s:_getbufvar')
  let display.setbufvar = function('s:_setbufvar')
  let display.setbufvar_batch = function('s:_setbufvar_batch')
  let display.cache = []
  let display.preload_capacity = 3 * &lines

  if s:is_nvim

    function! display.set_lines(lines) abort
      call clap#util#nvim_buf_set_lines(self.bufnr, a:lines)
    endfunction

    function! display.clear() abort
      call clap#util#nvim_buf_clear(self.bufnr)
    endfunction

    function! display.line_count() abort
      return nvim_buf_line_count(self.bufnr)
    endfunction

    function! display.append_lines(lines) abort
      call clap#util#nvim_buf_append_lines(self.bufnr, a:lines)
    endfunction

    function! display.append_lines_uncheck(lines) abort
      call self.append_lines(a:lines)
    endfunction

    function! display.first_line() abort
      return clap#util#nvim_buf_get_first_line(self.bufnr)
    endfunction

    function! display.clear_highlight() abort
      call self.goto_win()
      call self.matchdelete()
      call g:clap.input.goto_win()
    endfunction

    " Argument: list, multiple pattern to be highlighed
    function! display._apply_matchadd(patterns) abort
      call g:clap.display.goto_win()
      call s:matchadd(a:patterns)
      call g:clap.input.goto_win()
    endfunction

  else

    function! display.set_lines(lines) abort
      " silent is required to avoid the annoying --No lines in buffer--.
      silent call deletebufline(self.bufnr, 1, '$')

      call appendbufline(self.bufnr, 0, a:lines)
      " Delete the last possible empty line.
      " Is there a better solution in vim?
      if empty(getbufline(self.bufnr, '$')[0])
        silent call deletebufline(self.bufnr, '$')
      endif
    endfunction

    function! display.clear() abort
      silent call deletebufline(self.bufnr, 1, '$')
    endfunction

    function! display.line_count() abort
      " 8.1.1967
      return line('$', self.winid)
    endfunction

    " Due to the smart cache strategy, this should not be expensive.
    " :e nonexist.vim
    " :call appendbufline('', '$', [1, 2])
    "
    " 1:
    " 2: 1
    " 3: 2
    function! display.append_lines(lines) abort
      " call appendbufline(self.bufnr, '$', a:lines)
      " FIXME do not know why '$' doesn't work
      call appendbufline(self.bufnr, self.line_count() - 1, a:lines)
      " Is this check avoidable?
      if empty(getbufline(self.bufnr, '$')[0])
        silent call deletebufline(self.bufnr, '$')
      endif
    endfunction

    " Do not check the last line is empty or not.
    " It's safe for the non-empty files.
    function! display.append_lines_uncheck(lines) abort
      call appendbufline(self.bufnr, '$', a:lines)
    endfunction

    function! display.first_line() abort
      return get(getbufline(self.bufnr, 1), 0, '')
    endfunction

    function! display.clear_highlight() abort
      call win_execute(self.winid, 'call g:clap.display.matchdelete()')
    endfunction

    function! display._apply_matchadd(patterns) abort
      call win_execute(self.winid, 'call s:matchadd(a:patterns)')
    endfunction

  endif

  function! display.set_lines_lazy(raw_lines) abort
    if len(a:raw_lines) >= g:clap.display.preload_capacity
      let to_set = a:raw_lines[:g:clap.display.preload_capacity-1]
      let to_cache = a:raw_lines[g:clap.display.preload_capacity:]
      call self.set_lines(to_set)
      let g:clap.display.cache = to_cache
    else
      call self.set_lines(a:raw_lines)
    endif
  endfunction

  function! display.getcurline() abort
    return get(getbufline(self.bufnr, g:__clap_display_curlnum), 0, '')
  endfunction

  function! display.getcurlnum() abort
    " This seemingly doesn't work as expected.
    " return getbufinfo(winbufnr(self.winid))[0].lnum
    return g:__clap_display_curlnum
  endfunction

  function! display.is_empty() abort
    return self.line_count() == 1 && empty(self.first_line())
  endfunction

  " Optional argument: pattern to match
  " Default: input
  function! display.add_highlight(...) abort
    let pattern = a:0 > 0 ? a:1 : clap#filter#matchadd_pattern()
    if type(pattern) != v:t_list
      let pattern = [pattern]
    endif
    call self._apply_matchadd(pattern)
  endfunction

  function! display.matchdelete() abort
    if exists('w:clap_match_ids')
      call map(w:clap_match_ids, 'matchdelete(v:val)')
      unlet w:clap_match_ids
    endif
  endfunction

  return display
endfunction

function! s:init_input() abort
  let input = {}
  let input.getbufvar = function('s:_getbufvar')
  let input.setbufvar = function('s:_setbufvar')
  let input.setbufvar_batch = function('s:_setbufvar_batch')

  if s:is_nvim
    let input.goto_win = function('s:_goto_win')

    function! input.get() abort
      return clap#util#nvim_buf_get_first_line(self.bufnr)
    endfunction

    function! input.set(line) abort
      call setbufline(self.bufnr, 1, a:line)
    endfunction

    function! input.clear() abort
      call clap#util#nvim_buf_clear(self.bufnr)
    endfunction
  else
    function! input.goto_win() abort
      " Nothing happens
      " Vim popup is unfocuable.
    endfunction

    function! input.get() abort
      return clap#popup#get_input()
    endfunction

    function! input.set(line) abort
      call clap#popup#set_input(a:line)
    endfunction

    function! input.clear() abort
      call popup_settext(g:clap#popup#input.winid, '')
    endfunction
  endif

  return input
endfunction

function! s:init_provider() abort
  let provider = {}

  function! provider._() abort
    return g:clap.registrar[self.id]
  endfunction

  " Argument: String or List of String
  function! provider.abort(msg) abort
    if type(a:msg) == v:t_list
      let msg = string(a:msg)
    else
      let msg = a:msg
    endif
    throw 'clap:'.msg
  endfunction

  function! provider.sink(selected) abort
    call g:clap.start.goto_win()
    let Sink = self._().sink
    if type(Sink) == v:t_func
      call Sink(a:selected)
    elseif type(Sink) == v:t_string
      execute Sink a:selected
    else
      call clap#error("sink can only be a funcref or string.")
    endif
  endfunction

  function! provider.sink_star(lines) abort
    call self._()['sink*'](a:lines)
  endfunction

  function! provider.on_enter() abort
    if has_key(self._(), 'on_enter')
      call self._().on_enter()
    endif
  endfunction

  " After you have typed something
  function! provider.on_typed() abort
    try
      call self._().on_typed()
    catch
      call g:clap.display.set_lines(['provider.on_typed: '.v:exception])
      call clap#spinner#set_idle()
    endtry
  endfunction

  " When you press Ctrl-J/K
  function! provider.on_move() abort
    if has_key(self._(), 'on_move')
      call self._().on_move()
    endif
  endfunction

  function! provider.on_exit() abort
    if has_key(self._(), 'on_exit')
      call self._().on_exit()
    endif
  endfunction

  function! provider.jobstop() abort
    if has_key(self._(), 'jobstop')
      call self._().jobstop()
    endif
  endfunction

  function! provider.filter() abort
    return get(self._(), 'filter', v:null)
  endfunction

  function! provider.support_multi_selection() abort
    return has_key(self._(), 'sink*')
  endfunction

  function! provider.apply_args() abort
    if !empty(g:clap.provider.args)
          \ && g:clap.provider.args[0] !~# '^+'
      if s:is_nvim
        call feedkeys(join(g:clap.provider.args, ' '))
      else
        call g:clap.input.set(join(g:clap.provider.args, ' '))
      endif
      call clap#indicator#set_matches('')
      call g:clap.provider.on_typed()
    endif
  endfunction

  function! provider.source_async() abort
    if has_key(self._(), 'source_async')
      return self._().source_async()
    else
      call g:clap.abort("source_async is unavailable")
      return
    endif
  endfunction

  function! provider.get_source() abort
    let provider_info = self._()
    " Catch any exceptions and show them in the display window.
    try
      if has_key(provider_info, 'source')
        let Source = provider_info.source
        let source_ty = type(Source)
        if source_ty == v:t_func
          let lines = Source()
        elseif source_ty == v:t_list
          " Use copy here, otherwise it could be one-off List.
          let lines = copy(Source)
        elseif source_ty == v:t_string
          let lines = system(Source)
          if v:shell_error
            call clap#error('Fail to run '.Source)
            return ['Fail to run '.Source]
          endif
          return split(lines, "\n")
        else
          return ['provider.get_source: this should not happen, source can only be a list, string or funcref']
        endif
        return lines
      else
        return []
      endif
    catch
      call clap#spinner#set_idle()
      return ['provider.get_source: '.v:exception]
    endtry
  endfunction

  function! provider.is_sync() abort
    return has_key(self._(), 'source')
  endfunction

  function! provider.is_pure_async() abort
    return !has_key(self._(), 'source')
  endfunction

  function! provider.can_async() abort
    return !has_key(self._(), 'source') || has_key(self._(), 'source_async')
  endfunction

  function! provider.init_display_win() abort
    if self.is_pure_async()
      return
    endif
    let lines = self.get_source()
    let initial_size = len(lines)
    let g:clap.display.initial_size = initial_size
    if initial_size > 0
      call g:clap.display.set_lines_lazy(lines)
      call g:clap#display_win.compact_if_undersize()
      call clap#indicator#set_matches('['.initial_size.']')
    endif
  endfunction

  return provider
endfunction

function! s:extract(group, what, gui_or_cterm) abort
  return synIDattr(synIDtrans(hlID(a:group)), a:what, a:gui_or_cterm)
endfunction

function! s:extract_or(group, what, gui_or_cterm, default) abort
  let v = s:extract(a:group, a:what, a:gui_or_cterm)
  if empty(v)
    return a:default
  endif
  return v
endfunction

function! s:hi_display_invisible() abort
  " People can use their own display highlight group, so can't use s:display_default_hi_group here.
  let guibg = s:extract_or(s:display_group, 'bg', 'gui', '#544a65')
  let ctermbg = s:extract_or(s:display_group, 'bg', 'cterm', 60)
  execute printf(
        \ "hi ClapDisplayInvisibleEndOfBuffer ctermfg=%s guifg=%s",
        \ ctermbg,
        \ guibg
        \ )
endfunction

function! s:hi_preview_invisible() abort
  let guibg = s:extract_or(s:preview_group, 'bg', 'gui', '#5e5079')
  let ctermbg = s:extract_or(s:preview_group, 'bg', 'cterm', '60')
  execute printf(
        \ "hi ClapPreviewInvisibleEndOfBuffer ctermfg=%s guifg=%s",
        \ ctermbg,
        \ guibg
        \ )
endfunction

" Try to sync the spinner bg with input window.
function! s:hi_spinner() abort
  let vis_ctermbg = s:extract_or(s:input_default_hi_group, 'bg', 'cterm', '60')
  let vis_guibg = s:extract_or(s:input_default_hi_group, 'bg', 'gui', '#544a65')
  let fn_ctermfg = s:extract_or('Function', 'fg', 'cterm', '170')
  let fn_guifg = s:extract_or('Function', 'fg', 'gui', '#bc6ec5')
  execute printf(
        \ "hi ClapSpinner guifg=%s ctermfg=%s ctermbg=%s guibg=%s gui=bold cterm=bold",
        \ fn_guifg,
        \ fn_ctermfg,
        \ vis_ctermbg,
        \ vis_guibg,
        \ )

  let clap_sub_matches = [
        \ [173 , '#e18254'] ,
        \ [196 , '#f2241f'] ,
        \ [184 , '#e5d11c'] ,
        \ [32  , '#4f97d7'] ,
        \ [170 , '#bc6ec5'] ,
        \ [178 , '#ffbb7d'] ,
        \ [136 , '#b1951d'] ,
        \ [29  , '#2d9574'] ,
        \ ]

  let pmenu_ctermbg = s:extract_or(s:display_default_hi_group, 'bg', 'cterm', '60')
  let pmenu_guibg = s:extract_or(s:display_default_hi_group, 'bg', 'gui', '#544a65')

  let idx = 1
  for g in clap_sub_matches
    execute printf(
          \ "hi ClapMatches%s guifg=%s ctermfg=%s ctermbg=%s guibg=%s gui=bold cterm=bold", idx,
          \ g[1],
          \ g[0],
          \ pmenu_ctermbg,
          \ pmenu_guibg,
          \ )
    let idx += 1
  endfor
endfunction

function! s:init_hi_groups() abort
  if !hlexists('ClapSpinner')
    call s:hi_spinner()
    autocmd ColorScheme * call s:hi_spinner()
  endif

  if !hlexists('ClapInput')
    execute 'hi default link ClapInput' s:input_default_hi_group
  endif

  if !hlexists('ClapDisplay')
    execute 'hi default link ClapDisplay' s:display_default_hi_group
    let s:display_group = s:display_default_hi_group
  else
    let s:display_group = 'ClapDisplay'
  endif

  call s:hi_display_invisible()
  autocmd ColorScheme * call s:hi_display_invisible()

  if !hlexists('ClapPreview')
    execute 'hi default link ClapPreview' s:preview_defaualt_hi_group
    let s:preview_group = s:preview_defaualt_hi_group
  else
    let s:preview_group = 'ClapPreview'
  endif
  call s:hi_preview_invisible()
  autocmd ColorScheme * call s:hi_preview_invisible()

  " For the found matches highlight
  if !hlexists('ClapMatches')
    hi default link ClapMatches Search
  endif
  hi default link ClapQuery   IncSearch

  if !hlexists('ClapNoMatchesFound')
    hi default link ClapNoMatchesFound ErrorMsg
  endif
endfunction

function! clap#init#() abort
  let g:clap = {}
  let g:clap.is_busy = 0

  let g:clap.registrar = {}
  let g:clap.spinner = {}

  let g:clap.start = {}
  let g:clap.start.goto_win = function('s:_goto_win')
  let g:clap.start.get_lines = function('s:_get_lines')
  let g:clap.start.getbufvar = function('s:_getbufvar')
  let g:clap.start.setbufvar = function('s:_setbufvar')

  let g:clap.input = s:init_input()
  let g:clap.display = s:init_display()
  let g:clap.provider = s:init_provider()

  let g:clap.abort = g:clap.provider.abort

  call s:init_hi_groups()

  if s:is_nvim
    let g:clap.preview = g:clap#floating_win#preview
    let g:clap#display_win = g:clap#floating_win#display
    let g:clap.open_win = function('clap#floating_win#open')
    let g:clap.close_win = function('clap#floating_win#close')
  else
    let g:clap.preview = g:clap#popup#preview
    let g:clap#display_win = g:clap#popup#display
    let g:clap.open_win = function('clap#popup#open')
    let g:clap.close_win = function('clap#popup#close')
  endif
endfunction

let &cpo = s:save_cpo
unlet s:save_cpo
