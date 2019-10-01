" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Initialize and make a compatible layer between neovim and vim.

let s:save_cpo = &cpo
set cpo&vim

let s:is_nvim = has('nvim')
let s:default_priority = 10

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
    function! display.append_lines(lines) abort
      " call appendbufline(self.bufnr, '$', a:lines)
      " FIXME do not know why '$' doesn't work
      call appendbufline(self.bufnr, self.line_count() - 1, a:lines)
      " Is this check avoidable?
      if empty(getbufline(self.bufnr, '$')[0])
        silent call deletebufline(self.bufnr, '$')
      endif
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

  function! provider.abort(msg) abort
    throw 'clap: '.a:msg
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
          call clap#error('source must be a list, string or funcref')
          return ['source can only be a list, string or funcref']
        endif
        return lines
      elseif self.is_sync()
        return ['provider.get_source: No source, this should not happen.']
      endif
    catch
      call clap#spinner#set_idle()
      return ['provider.get_source: '.v:exception]
    endtry
  endfunction

  function! provider.is_sync() abort
    return has_key(self._(), 'source')
  endfunction

  function! provider.is_async() abort
    return !has_key(self._(), 'source')
  endfunction

  function! provider.init_display_win() abort
    let lines = self.get_source()
    if !empty(lines)
      call g:clap.display.set_lines(lines)
      call g:clap#display_win.compact_if_undersize()
    endif
  endfunction

  return provider
endfunction

function! s:extract(group, what, ...) abort
  if a:0 == 1
    return synIDattr(synIDtrans(hlID(a:group)), a:what, a:1)
  else
    return synIDattr(synIDtrans(hlID(a:group)), a:what)
  endif
endfunction

function! s:hi_display_invisible() abort
  let guibg = s:extract(s:display_group, 'bg', 'gui')
  let ctermbg = s:extract(s:display_group, 'bg', 'cterm')
  execute printf(
        \ "hi ClapDisplayInvisibleEndOfBuffer ctermfg=%s guifg=%s",
        \ ctermbg,
        \ guibg
        \ )
endfunction

function! s:hi_preview_invisible() abort
  let guibg = s:extract(s:preview_group, 'bg', 'gui')
  let ctermbg = s:extract(s:preview_group, 'bg', 'cterm')
  execute printf(
        \ "hi ClapPreviewInvisibleEndOfBuffer ctermfg=%s guifg=%s",
        \ ctermbg,
        \ guibg
        \ )
endfunction

" Try to sync the spinner bg with input window.
function! s:hi_spinner() abort
  let vis_ctermbg = s:extract('Visual', 'bg', 'cterm')
  if empty(vis_ctermbg)
    let vis_ctermbg = '60'
  endif
  let vis_guibg = s:extract('Visual', 'bg', 'gui')
  if empty(vis_guibg)
    let vis_guibg = '#544a65'
  endif
  let fn_ctermfg = s:extract('Function', 'fg', 'cterm')
  if empty(fn_ctermfg)
    let fn_ctermfg = '170'
  endif
  let fn_guifg = s:extract('Function', 'fg', 'gui')
  if empty(fn_guifg)
    let fn_guifg = '#bc6ec5'
  endif
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

  let pmenu_ctermbg = s:extract('Pmenu', 'bg', 'cterm')
  if empty(pmenu_ctermbg)
    let pmenu_ctermbg = '60'
  endif
  let pmenu_guibg = s:extract('Pmenu', 'bg', 'gui')
  if empty(pmenu_guibg)
    let pmenu_guibg = '#544a65'
  endif

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
    hi default link ClapInput Visual
  endif

  if !hlexists('ClapDisplay')
    hi default link ClapDisplay Pmenu
    let s:display_group = 'Pmenu'
  else
    let s:display_group = 'ClapDisplay'
  endif

  call s:hi_display_invisible()
  autocmd ColorScheme * call s:hi_display_invisible()

  if !hlexists('ClapPreview')
    hi default link ClapPreview PmenuSel
    let s:preview_group = 'PmenuSel'
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
