" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Make a compatible layer between neovim and vim.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:is_nvim = has('nvim')
let s:cat_or_type = has('win32') ? 'type' : 'cat'

function! s:_goto_win() dict abort
  call win_gotoid(self.winid)
endfunction

function! s:_getbufvar(varname) dict abort
  return getbufvar(self.bufnr, a:varname)
endfunction

function! s:_setbufvar(varname, val) dict abort
  call setbufvar(self.bufnr, a:varname, a:val)
endfunction

function! s:_setbufvar_batch(dict) dict abort
  call map(a:dict, 'setbufvar(self.bufnr, v:key, v:val)')
endfunction

function! s:_system(cmd) abort
  let lines = systemlist(a:cmd)
  if v:shell_error
    return ['Fail to call system('.a:cmd.')']
  endif
  return lines
endfunction

if s:is_nvim
  function! s:_get_lines() dict abort
    return nvim_buf_get_lines(self.bufnr, 0, -1, 0)
  endfunction

  function! s:_line_count() dict abort
    return nvim_buf_line_count(self.bufnr)
  endfunction

  function! s:_win_is_valid() dict abort
    return self.winid == -1 ? v:false : nvim_win_is_valid(self.winid)
  endfunction
else
  function! s:_get_lines() dict abort
    let lines = getbufline(self.bufnr, 0, '$')
    return len(lines) == 1 && empty(lines[0]) ? [] : lines
  endfunction

  function! s:_line_count() dict abort
    " 8.1.1967
    return line('$', self.winid)
  endfunction

  function! s:_win_is_valid() dict abort
    return !empty(popup_getpos(self.winid))
  endfunction
endif

function! s:init_display() abort
  let display = {}
  call s:inject_base_api(display)
  let display.cache = []
  let display.preload_capacity = 2 * &lines

  if s:is_nvim

    function! display.set_cursor(lnum, col) abort
      call nvim_win_set_cursor(self.winid, [a:lnum, a:col])
    endfunction

    function! display.clear() abort
      call clap#api#buf_clear(self.bufnr)
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

    if exists('*win_execute')
      function! display.clear_highlight() abort
        call win_execute(self.winid, 'noautocmd call self.matchdelete()')
      endfunction

      function! display.legacy_apply_matchadd(patterns) abort
        call win_execute(self.winid, 'call clap#legacy#highlighter#highlight_substring(a:patterns)')
      endfunction
    else
      function! display.clear_highlight() abort
        noautocmd call self.goto_win()
        " Clear all matches added in the display window
        "
        " We should not use clearmatches() as it will clear the
        " ClapNoMatchesFound highlight as well.
        "
        " call clearmatches()
        call self.matchdelete()
        noautocmd call g:clap.input.goto_win()
      endfunction

      " Argument: list, multiple pattern to be highlighed
      function! display.legacy_apply_matchadd(patterns) abort
        call g:clap.display.goto_win()
        call clap#legacy#highlighter#highlight_substring(a:patterns)
        call g:clap.input.goto_win()
      endfunction
    endif

  else

    function! display.set_cursor(lnum, col) abort
      call win_execute(self.winid, 'call cursor(a:lnum, a:col)')
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
      if empty(get(getbufline(self.bufnr, '$'), 0, ''))
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

    function! display.legacy_apply_matchadd(patterns) abort
      call win_execute(self.winid, 'call clap#legacy#highlighter#highlight_substring(a:patterns)')
    endfunction

  endif

  function! display.set_lines(lines) abort
    call clap#api#buf_set_lines(self.bufnr, a:lines)
  endfunction

  function! display.clear() abort
    call clap#api#buf_clear(self.bufnr)
  endfunction

  function! display.set_lines_lazy(raw_lines) abort
    if len(a:raw_lines) >= g:clap.display.preload_capacity
      let to_set = a:raw_lines[:g:clap.display.preload_capacity-1]
      let to_cache = a:raw_lines[g:clap.display.preload_capacity : ]
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
    return clap#api#get_origin_line_at(g:__clap_display_curlnum)
  endfunction

  function! display.deletecurline() abort
    call deletebufline(self.bufnr, g:__clap_display_curlnum)
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
  function! display.legacy_add_highlight(...) abort
    let pattern = a:0 > 0 ? a:1 : clap#legacy#filter#sync#viml#matchadd_pattern()
    if empty(pattern)
      return
    endif
    if type(pattern) != v:t_list
      let pattern = [pattern]
    endif
    call self.legacy_apply_matchadd(pattern)
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
  call s:inject_base_api(input)

  if s:is_nvim
    let input.goto_win = function('s:_goto_win')

    function! input.get() abort
      return clap#util#nvim_buf_get_first_line(self.bufnr)
    endfunction

    function! input.set(line) abort
      call setbufline(self.bufnr, 1, a:line)
    endfunction

    function! input.clear() abort
      call clap#api#buf_clear(self.bufnr)
    endfunction
  else
    function! input.goto_win() abort
      " Nothing happens
      " Vim popup is unfocuable.
    endfunction

    function! input.get() abort
      return clap#popup#move_manager#get_input()
    endfunction

    function! input.set(line) abort
      call clap#popup#move_manager#set_input(a:line)
    endfunction

    function! input.clear() abort
      call popup_settext(self.winid, '')
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
    throw 'clap:'.string(a:msg)
  endfunction

  function! provider._apply_sink(selected) abort
    let Sink = self._().sink
    if type(Sink) == v:t_func
      call Sink(a:selected)
    elseif type(Sink) == v:t_string
      execute Sink a:selected
    else
      call clap#helper#echo_error('sink can only be a funcref or string.')
    endif
  endfunction

  function! provider.has_enable_rooter() abort
    return get(self._(), 'enable_rooter', v:false)
  endfunction

  function! provider.sink(selected) abort
    call clap#rooter#run_sink_or_sink_star(self._apply_sink, a:selected)
  endfunction

  function! provider.sink_star(lines) abort
    call clap#rooter#run_sink_or_sink_star(self._()['sink*'], a:lines)
  endfunction

  function! provider.on_enter() abort
    if has_key(self._(), 'on_enter')
      call self._().on_enter()
    endif
  endfunction

  " After you have typed something
  function! provider.on_typed() abort
    " If ++query is being used, we should do `on_typed`, ref #515.
    if get(g:, '__clap_open_win_pre', v:false) && !has_key(g:clap.context, 'query')
      return
    endif
    try
      call clap#sign#reset_on_query_change()
      call self._().on_typed()
      call clap#preview#update_with_delay()
    catch
      let l:error_info = ['provider.on_typed:'] + split(v:throwpoint, '\[\d\+\]\zs') + split(v:exception, "\n")
      call g:clap.display.set_lines(l:error_info)
      call g:clap#display_win.shrink()
      call clap#spinner#set_idle()
    endtry
  endfunction

  " When you press Ctrl-J/K
  function! provider.on_move() abort
    call clap#impl#on_move#invoke()
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

  function! provider.support_multi_select() abort
    return has_key(self._(), 'multi_select') || has_key(self._(), 'sink*') || has_key(get(self._(), 'mappings', {}), "<Tab>")
  endfunction

  function! provider.support_open_action() abort
    return get(self._(), 'support_open_action', v:false)
  endfunction

  function! provider.is_rpc_type() abort
    return has_key(self._(), 'source_type') && self._().source_type == g:__t_rpc
  endfunction

  function! provider.try_set_syntax() abort
    if has_key(self._(), 'syntax')
      call g:clap.display.setbufvar('&syntax', self._().syntax)
    endif
  endfunction

  function! provider._apply_source() abort
    let ClapProviderSource = self._().source

    if self.source_type == g:__t_string
      return s:_system(ClapProviderSource)
    elseif self.source_type == g:__t_list
      " Use copy here, otherwise it could be one-off List.
      return copy(ClapProviderSource)
    elseif self.source_type == g:__t_func_string
      return s:_system(ClapProviderSource())
    elseif self.source_type == g:__t_func_list
      return copy(ClapProviderSource())
    else
      return ['source() must return a List or a String if it is a Funcref']
    endif
  endfunction

  function! provider.is_sync() abort
    return has_key(self._(), 'source')
  endfunction

  function! provider.is_pure_async() abort
    return !has_key(self._(), 'source')
  endfunction

  function! provider.init_display_win() abort
    if has_key(self._(), 'init')
      call self._().init()
    else
      " Still create a new session on the Rust side for the general on_move impl.
      call clap#client#notify_on_init()
    endif
  endfunction

  function! provider.mode() abort
    return get(self._(), 'mode', 'full')
  endfunction

  return provider
endfunction

function! s:inject_base_api(dict) abort
  let dict = a:dict
  let dict.line_count = function('s:_line_count')
  let dict.win_is_valid = function('s:_win_is_valid')
  let dict.goto_win = function('s:_goto_win')
  let dict.get_lines = function('s:_get_lines')
  let dict.getbufvar = function('s:_getbufvar')
  let dict.setbufvar = function('s:_setbufvar')
  let dict.setbufvar_batch = function('s:_setbufvar_batch')
endfunction

function! s:matchaddpos(highlight_line) abort
  if exists('w:clap_preview_hi_id')
    call matchdelete(w:clap_preview_hi_id)
  endif
  if type(a:highlight_line) == v:t_number
    let w:clap_preview_hi_id = matchaddpos('Search', [[a:highlight_line]])
  else
    if has_key(a:highlight_line, 'column_range')
      let w:clap_preview_hi_id = matchaddpos('Search', [[a:highlight_line.line_number, a:highlight_line.column_range.start, a:highlight_line.column_range.end - a:highlight_line.column_range.start]])
    else
      let w:clap_preview_hi_id = matchaddpos('Search', [[a:highlight_line.line_number]])
    endif
  endif
endfunction

function! clap#api#clap#init() abort
  let g:clap = {}

  let g:clap.registrar = {}
  let g:clap.spinner = {}

  let g:clap.start = {}
  call s:inject_base_api(g:clap.start)

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

  function! g:clap.preview.set_syntax(syntax) abort
    call g:clap.preview.setbufvar('&syntax', a:syntax)
  endfunction

  if exists('*win_execute')
    function! g:clap.preview.add_highlight(highlight_line) abort
      call win_execute(g:clap.preview.winid, 'noautocmd call s:matchaddpos(a:highlight_line)')
    endfunction
  else
    function! g:clap.preview.add_highlight(highlight_line) abort
      noautocmd call win_gotoid(g:clap.preview.winid)
      call s:matchaddpos(a:highlight_line)
      noautocmd call win_gotoid(g:clap.input.winid)
    endfunction
  endif

  call s:inject_base_api(g:clap.preview)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
