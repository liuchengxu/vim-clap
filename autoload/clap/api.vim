" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Make a compatible layer between neovim and vim.

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
      " An empty buffer consists of one empty line. If you append, this line is still there.
      " https://github.com/vim/vim/issues/5016
      " Thus this is unavoidable.
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
      " b -> b0
      " Continuing to input more chars leads to the number of filtered result smaller,
      " in which case the get_lines() could overlap with current cache, thus
      " we should not use the cache next time.
      let g:__clap_do_not_use_cache = v:true
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

  function! provider._apply_sink(selected) abort
    let Sink = self._().sink
    if type(Sink) == v:t_func
      call Sink(a:selected)
    elseif type(Sink) == v:t_string
      execute Sink a:selected
    else
      call clap#error("sink can only be a funcref or string.")
    endif
  endfunction

  function! provider.sink(selected) abort
    call g:clap.start.goto_win()
    " FIXME what if the sink function changes cwd intentionally? Then we
    " should not restore to the current cwd after executing the sink function.
    call clap#util#run_from_project_root(self._apply_sink, a:selected)
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

  function! provider.source_async_or_default() abort
    if has_key(self._(), 'source_async')
      return self._().source_async()
    else
      let Source = self._().source
      let source_ty = type(Source)
      if source_ty == v:t_string
        let ext_filter_cmd = clap#filter#get_external_cmd_or_default()
        " FIXME Does it work well in Windows?
        let cmd = Source.' | '.ext_filter_cmd
        return cmd
      endif

      if source_ty == v:t_func
        let lines = Source()
      elseif source_ty == v:t_list
        let lines = copy(Source)
      else
        call g:clap.abort("source_ty is neither func nor list, this should not happen")
        return
      endif
      let tmp = tempname()
      if writefile(lines, tmp) == 0
        let ext_filter_cmd = clap#filter#get_external_cmd_or_default()
        let cat_or_type = has('win32') ? 'type' : 'cat'
        let cmd = printf('%s %s | %s', cat_or_type, tmp, ext_filter_cmd)
        call add(g:clap.tmps, tmp)
        return cmd
      else
        call g:clap.abort("Fail to write source to a temp file")
        return
      endif
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

  function! provider._apply_source() abort
    let Source = self._().source
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
  endfunction

  function! provider.get_source() abort
    let provider_info = self._()
    " Catch any exceptions and show them in the display window.
    try
      if has_key(provider_info, 'source')
        return clap#util#run_from_project_root(self._apply_source)
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

  " A provider can be async if it's pure async or sync provider with `source_async`
  " Since now we have the default source_async implementation, everything
  " could be async theoretically.
  "
  " But the default async impl may not work in Windows at the moment,
  " So we have a flag for people to disable it.
  function! provider.can_async() abort
    return !get(g:, 'clap_disable_optional_async', v:false)
  endfunction

  function! provider.init_display_win() abort
    if self.is_pure_async()
      return
    endif
    " Even for the syn providers that could have 10,000+ lines, it's ok to show it now.
    let lines = self.get_source()
    let initial_size = len(lines)
    let g:clap.display.initial_size = initial_size
    if initial_size > 0
      call g:clap.display.set_lines_lazy(lines)
      call g:clap#display_win.compact_if_undersize()
      call clap#indicator#set_matches('['.initial_size.']')
      call clap#sign#toggle_cursorline()
    endif
  endfunction

  return provider
endfunction

function! clap#api#bake() abort
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
