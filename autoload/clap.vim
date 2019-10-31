" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Primiary code path for the plugin.

let s:save_cpo = &cpoptions
set cpoptions&vim

if has('nvim')
  let s:has_features = has('nvim-0.4')
else
  let s:has_features = has('patch-8.1.2114')
endif

if !s:has_features
  echoerr 'Vim-clap requires NeoVim >= 0.4.0 or Vim 8.1.2114+'
  echoerr 'Please upgrade your editor accordingly.'
  finish
endif

let s:cur_dir = fnamemodify(resolve(expand('<sfile>:p')), ':h')
let s:builtin_providers = map(
      \ split(globpath(s:cur_dir.'/clap/provider', '*'), '\n'),
      \ 'fnamemodify(v:val, '':t:r'')'
      \ )

let g:__clap_dir = s:cur_dir

let g:clap#builtin_providers = s:builtin_providers

let g:__t_func = 0
let g:__t_string = 1
let g:__t_list = 2
let g:__t_func_string = 3
let g:__t_func_list = 4

let s:provider_alias = {
      \ 'hist:': 'command_history',
      \ 'gfiles': 'git_files',
      \ }

let s:provider_alias = extend(s:provider_alias, get(g:, 'clap_provider_alias', {}))

let g:clap_no_matches_msg = get(g:, 'clap_no_matches_msg', 'NO MATCHES FOUND')
let g:__clap_no_matches_pattern = '^'.g:clap_no_matches_msg.'$'

let s:default_symbols = {
      \ 'arrow' : ["\ue0b2", "\ue0b0"],
      \ 'curve' : ["\ue0b6", "\ue0b4"],
      \ 'nil'   : ['', ''],
      \ }

let g:clap_search_box_border_symbols = extend(s:default_symbols, get(g:, 'clap_search_box_border_symbols', {}))
let g:clap_search_box_border_style = get(g:, 'clap_search_box_border_style',
      \ exists('g:spacevim_nerd_fonts') || exists('g:airline_powerline_fonts') ? 'curve' : 'nil')
let g:__clap_search_box_border_symbol = {
      \ 'left': get(g:clap_search_box_border_symbols, g:clap_search_box_border_style, '')[0],
      \ 'right': get(g:clap_search_box_border_symbols, g:clap_search_box_border_style, '')[1],
      \ }

let s:default_action = {
  \ 'ctrl-t': 'tab split',
  \ 'ctrl-x': 'split',
  \ 'ctrl-v': 'vsplit',
  \ }

let g:clap_open_action = get(g:, 'clap_open_action', s:default_action)

function! clap#error(msg) abort
  echohl ErrorMsg
  echom '[vim-clap] '.a:msg
  echohl NONE
endfunction

function! s:inject_default_impl_is_ok(provider_info) abort
  let provider_info = a:provider_info

  " If sync provider
  if has_key(provider_info, 'source')
    if !has_key(provider_info, 'on_typed')
      let provider_info.on_typed = function('clap#impl#on_typed')
    endif
    if !has_key(provider_info, 'filter')
      let provider_info.filter = function('clap#filter#')
    endif
  else
    if !has_key(provider_info, 'on_typed')
      call clap#error('Provider without source must specify on_moved, but only has: '.keys(provider_info))
      return v:false
    endif
    if !has_key(provider_info, 'jobstop')
      let provider_info.jobstop = function('clap#dispatcher#jobstop')
    endif
  endif

  return v:true
endfunction

function! s:_sink(selected) abort
  let provider = matchstr(a:selected, '^\(.*\)\ze:')
  " a sink for "Clap _" (dispatch to other builtin clap providers).
  call timer_start(0, {-> clap#_for(provider)})
endfunction

function! clap#_init() abort
  if has_key(g:clap.provider._(), 'source')
    let Source = g:clap.provider._().source
    let source_ty = type(Source)

    if source_ty == v:t_string
      let g:clap.provider.type = g:__t_string
    elseif source_ty == v:t_list
      let g:clap.provider.type = g:__t_list
    elseif source_ty == v:t_func
      let string_or_list = Source()
      if type(string_or_list) == v:t_string
        let g:clap.provider.type = g:__t_func_string
      elseif type(string_or_list) == v:t_list
        let g:clap.provider.type = g:__t_func_list
      else
        call g:clap.abort('Must return a String or a List if source is a Funcref')
        return
      endif
    endif
  endif

  call clap#spinner#init()

  call g:clap.provider.init_display_win()
endfunction

function! clap#_exit() abort
  call g:clap.provider.on_exit()
  call g:clap.provider.jobstop()

  call g:clap.close_win()

  let g:clap.is_busy = 0
  let g:clap.display.cache = []

  call g:clap.input.clear()
  call g:clap.display.clear()

  if has_key(g:clap.provider, 'args')
    call remove(g:clap.provider, 'args')
  endif

  if exists('g:__clap_fuzzy_matched_indices')
    unlet g:__clap_fuzzy_matched_indices
  endif

  call clap#sign#reset()

  call map(g:clap.tmps, 'delete(v:val)')
  let g:clap.tmps = []

  silent doautocmd <nomodeline> User ClapOnExit
endfunction

function! clap#_for(provider_id_or_alias) abort
  let g:clap.provider.args = []
  call clap#for(a:provider_id_or_alias)
endfunction

" Sometimes we don't need to go back to the start window, hence clap#_exit() is extracted.
function! clap#exit() abort
  " NOTE: Need to go back to the start window
  call g:clap.start.goto_win()

  call clap#_exit()
endfunction

function! clap#complete(A, L, P) abort
  let registered = exists('g:clap') ? keys(g:clap.registrar) : []
  return join(uniq(sort(s:builtin_providers + keys(s:provider_alias) + registered)), "\n")
endfunction

function! clap#register(provider_id, provider_info) abort
  let provider_info = a:provider_info

  if has_key(g:clap.registrar, a:provider_id)
    call clap#error('This provider id already exists: '.a:provider_id)
    return
  endif

  if !s:inject_default_impl_is_ok(provider_info)
    return
  endif

  let g:clap.registrar[a:provider_id] = provider_info
endfunction

function! s:validate_provider(registration_info) abort
  " Every provider should specify the sink option.
  if !has_key(a:registration_info, 'sink')
    call clap#error('A valid provider must provide sink option')
    return v:false
  endif
  if has_key(a:registration_info, 'source')
    let ty_source = type(a:registration_info.source)
    if ty_source == v:t_list
          \ || ty_source == v:t_string
          \ || ty_source == v:t_func
    else
      call clap#error('source must be a list, string or funcref')
      return v:false
    endif
  else
    " Pure async provider
    if !has_key(a:registration_info, 'on_typed')
      call clap#error('An async provider must provide on_typed option')
      return v:false
    endif
  endif
  return v:true
endfunction

function! s:try_register_is_ok(provider_id) abort
  let provider_id = a:provider_id

  " User pre-defined config in the vimrc
  if exists('g:clap_provider_{provider_id}')
    let registration_info = g:clap_provider_{provider_id}
  else
    " Try the autoloaded provider
    try
      let registration_info = g:clap#provider#{provider_id}#
    catch /^Vim\%((\a\+)\)\=:E121/
      call clap#error('Fail to load the provider: '.provider_id)
      return v:false
    endtry
  endif

  if !s:inject_default_impl_is_ok(registration_info)
    return v:false
  endif

  let g:clap.registrar[provider_id] = {}
  call extend(g:clap.registrar[provider_id], registration_info)

  return s:validate_provider(registration_info)
endfunction

function! clap#for(provider_id_or_alias) abort
  if has_key(s:provider_alias, a:provider_id_or_alias)
    let provider_id = s:provider_alias[a:provider_id_or_alias]
  else
    let provider_id = a:provider_id_or_alias
  endif

  let g:clap.provider.id = provider_id
  let g:clap.display.cache = []

  " If the registrar is not aware of this provider, try registering it.
  if !has_key(g:clap.registrar, provider_id) && !s:try_register_is_ok(provider_id)
    return
  endif

  call clap#handler#init()

  call g:clap.open_win()
endfunction

function! s:_source() abort
  if !exists('s:global_source')
    let s:global_source = []
    for provider_id in s:builtin_providers
      let provider_path = globpath(&runtimepath, 'autoload/clap/provider/'.provider_id.'.vim')
      if file_readable(provider_path)
        let desc_line = readfile(provider_path, '', 2)[-1]
        let desc = matchstr(desc_line, '^.*Description: \zs\(.*\)\ze\.\?$')
        if empty(desc)
          call add(s:global_source, provider_id.':')
        else
          call add(s:global_source, provider_id.': '.desc)
        endif
      endif
    endfor
  endif
  return s:global_source
endfunction

if !exists('g:clap')
  call clap#init#()
  call clap#register('_', {
        \ 'source': function('s:_source'),
        \ 'sink': function('s:_sink'),
        \ 'on_enter': { -> g:clap.display.setbufvar('&ft', 'clap_global') },
        \ 'on_exit': { -> g:clap.display.setbufvar('&ft', '') },
        \ })
endif

function! s:parse_opts(args) abort
  let idx = 0
  for arg in a:args
    if arg =~? '^++\w*=\w*'
      let matched = matchlist(arg, '^++\(\w*\)=\(\S*\)')
      let [k, v] = [matched[1], matched[2]]
      let g:clap.context[k] = v
    elseif arg =~? '^+\w*'
      let opt = arg[1:]
      let g:clap.context[opt] = v:true
    else
      break
    endif
    let idx += 1
  endfor
  if has_key(g:clap.context, 'query')
    let g:clap.context.query = clap#util#expand(g:clap.context.query)
  endif
  let g:clap.provider.args = a:args[idx :]
endfunction

function! clap#(bang, ...) abort
  let g:clap.start.bufnr = bufnr('')
  let g:clap.start.winid = win_getid()
  let g:clap.start.old_pos = getpos('.')

  let g:clap.context = {}
  let g:clap.tmps = []

  if a:bang
    let g:clap.context.async = v:true
  endif

  if a:0 == 0
    let provider_id_or_alias = '_'
    let g:clap.provider.args = []
  else
    if a:000 == ['debug']
      call clap#debugging#info()
      return
    elseif a:000 == ['debug+']
      call clap#debugging#info_to_clipboard()
      return
    endif
    let provider_id_or_alias = a:1
    call s:parse_opts(a:000[1:])
  endif

  call clap#for(provider_id_or_alias)
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
