" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Primiary code path for the plugin.

scriptencoding utf-8

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:has_features = has('nvim') ? has('nvim-0.4') : has('patch-8.1.2114')

if !s:has_features
  echoerr 'Vim-clap requires NeoVim >= 0.4.0 or Vim 8.1.2114+'
  echoerr 'Please upgrade your editor accordingly.'
  finish
endif

let s:cur_dir = fnamemodify(resolve(expand('<sfile>:p')), ':h')

let g:clap#autoload_dir = s:cur_dir

let g:__t_func = 0
let g:__t_string = 1
let g:__t_list = 2
let g:__t_func_string = 3
let g:__t_func_list = 4
let g:__t_rpc = 5

let s:provider_alias = {
      \ 'hist:': 'command_history',
      \ 'hist/': 'search_history',
      \ 'gfiles': 'git_files',
      \ }

let s:provider_alias = extend(s:provider_alias, get(g:, 'clap_provider_alias', {}))
let g:clap#provider_alias = s:provider_alias
let g:clap_disable_run_rooter = get(g:, 'clap_disable_run_rooter', v:false)
let g:clap_disable_bottom_top = get(g:, 'clap_disable_bottom_top', 0)
let g:clap_enable_debug = get(g:, 'clap_enable_debug', v:false)
let g:clap_forerunner_status_sign = get(g:, 'clap_forerunner_status_sign', {'done': '•', 'running': '!', 'using_cache': '*'})

" Backward compatible
if exists('g:clap_forerunner_status_sign_done')
  let g:clap_forerunner_status_sign.done = g:clap_forerunner_status_sign_done
endif

if exists('g:clap_forerunner_status_sign_running')
  let g:clap_forerunner_status_sign.running = g:clap_forerunner_status_sign_running
endif

let g:clap_disable_optional_async = get(g:, 'clap_disable_optional_async', v:false)
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

let g:clap_open_preview = get(g:, 'clap_open_preview', 'always')
let g:clap_open_action = get(g:, 'clap_open_action', s:default_action)
let g:clap_enable_icon = get(g:, 'clap_enable_icon', exists('g:loaded_webdevicons') || get(g:, 'spacevim_nerd_fonts', 0))
let g:clap_preview_size = get(g:, 'clap_preview_size', 5)
let g:clap_preview_direction = get(g:, 'clap_preview_direction', 'AUTO')
let g:clap_enable_background_shadow = get(g:, 'clap_enable_background_shadow', v:true)
let g:clap_background_shadow_blend = get(g:, 'clap_background_shadow_blend', 50)
let g:clap_insert_mode_only = get(g:, 'clap_insert_mode_only', v:false)
let g:clap_providers_relaunch_code = get(g:, 'clap_providers_relaunch_code', '@@')
let g:clap_disable_matches_indicator = get(g:, 'clap_disable_matches_indicator', v:false)
let g:clap_multi_selection_warning_silent = get(g:, 'clap_multi_selection_warning_silent', 0)

" Do not use this unless you know what you are doing!!!
let g:__clap_development = get(g:, '__clap_development', v:false)

let g:clap_popup_border = get(g:, 'clap_popup_border', has('nvim') ? 'single' : 'rounded')

function! clap#builtin_providers() abort
  if !exists('s:builtin_providers')
    let s:builtin_providers = map(
          \ split(globpath(s:cur_dir.'/clap/provider', '*'), '\n'),
          \ 'fnamemodify(v:val, '':t:r'')'
          \ )
  endif
  return s:builtin_providers
endfunction

function! s:inject_default_impl_is_ok(provider_info) abort
  let provider_info = a:provider_info

  " If sync provider
  if has_key(provider_info, 'source')
    if !has_key(provider_info, 'on_typed')
      let provider_info.on_typed = function('clap#impl#on_typed#')
    endif
    if !has_key(provider_info, 'filter')
      let provider_info.filter = function('clap#filter#sync')
    endif
  else
    if !has_key(provider_info, 'on_typed')
      call clap#helper#echo_error('Provider without source must specify on_moved, but only has: '.keys(provider_info))
      return v:false
    endif
    if !has_key(provider_info, 'jobstop')
      let provider_info.jobstop = function('clap#dispatcher#jobstop')
    endif
  endif

  return v:true
endfunction

function! s:detect_source_type() abort
  let Source = g:clap.provider._().source
  let source_ty = type(Source)

  if source_ty == v:t_string
    return g:__t_string
  elseif source_ty == v:t_list
    return g:__t_list
  elseif source_ty == v:t_func
    let string_or_list = Source()
    if type(string_or_list) == v:t_string
      return g:__t_func_string
    elseif type(string_or_list) == v:t_list
      return g:__t_func_list
    else
      call g:clap.abort('Must return a String or a List if source is a Funcref')
    endif
  endif
  return v:null
endfunction

function! clap#_init() abort
  call clap#spinner#init()

  call g:clap.provider.init_display_win()

  " Ensure the filetype is empty on init.
  " Each provider can set its own syntax for the highlight purpose.
  call g:clap.display.setbufvar('&filetype', '')
endfunction

function! clap#_exit() abort
  call g:clap.provider.jobstop()
  call clap#job#regular#forerunner#stop()
  call clap#maple#clean_up()

  noautocmd call g:clap.close_win()
  call g:clap.preview.clear()

  let g:clap.display.cache = []
  let g:clap.display.initial_size = -1
  " Reset this for vim issue. Ref #223
  let g:clap.display.winid = -1

  " Remember to get what the sink needs before clearing the buffer.
  call g:clap.input.clear()
  call g:clap.display.clear()

  call clap#sign#reset()

  call clap#state#clear_post()

  call map(g:clap.tmps, 'delete(v:val)')
  let g:clap.tmps = []
endfunction

function! clap#_for(provider_id_or_alias) abort
  let g:clap.provider.args = []
  call clap#for(a:provider_id_or_alias)
endfunction

" Sometimes we don't need to go back to the start window, hence clap#_exit() is extracted.
function! clap#exit() abort
  call g:clap.start.goto_win()
  call clap#_exit()
endfunction

function! clap#should_use_raw_cwd() abort
  return g:clap_disable_run_rooter
        \ || !g:clap.provider.has_enable_rooter()
        \ || getbufvar(g:clap.start.bufnr, '&bt') ==# 'terminal'
endfunction

function! clap#register(provider_id, provider_info) abort
  if has_key(g:clap.registrar, a:provider_id)
    call clap#helper#echo_error('This provider id already exists: '.a:provider_id)
    return
  endif

  if !s:inject_default_impl_is_ok(a:provider_info)
    return
  endif

  let g:clap.registrar[a:provider_id] = a:provider_info
endfunction

function! s:validate_provider(registration_info) abort
  " Every provider should specify the sink option.
  if !has_key(a:registration_info, 'sink')
    call clap#helper#echo_error('A valid provider must provide sink option')
    return v:false
  endif
  if has_key(a:registration_info, 'source')
    let ty_source = type(a:registration_info.source)
    if ty_source == v:t_list
          \ || ty_source == v:t_string
          \ || ty_source == v:t_func
    else
      call clap#helper#echo_error('source must be a list, string or funcref')
      return v:false
    endif
  else
    " Pure async provider
    if !has_key(a:registration_info, 'on_typed')
      call clap#helper#echo_error('An async provider must provide on_typed option')
      return v:false
    endif
  endif
  return v:true
endfunction

function! s:try_register_is_ok(provider_id) abort
  " User pre-defined config in the vimrc
  if exists('g:clap_provider_{a:provider_id}')
    let registration_info = g:clap_provider_{a:provider_id}
  else
    " Try the autoloaded provider
    try
      let registration_info = g:clap#provider#{a:provider_id}#
    catch /^Vim\%((\a\+)\)\=:E121/
      call clap#helper#echo_error('Fail to load provider: '.a:provider_id.', E:'.v:exception)
      return v:false
    endtry
  endif

  if !s:inject_default_impl_is_ok(registration_info)
    return v:false
  endif

  let g:clap.registrar[a:provider_id] = {}
  call extend(g:clap.registrar[a:provider_id], registration_info)

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
  if !has_key(g:clap.registrar, provider_id)
        \ && !s:try_register_is_ok(provider_id)
    return
  endif

  call clap#state#clear_pre()

  " g:__clap_provider_cwd can be set during this process, so this needs to be executed after s:clear_state()
  if has_key(g:clap.provider._(), 'source')
    if has_key(g:clap.provider._(), 'source_type')
      let g:clap.provider.source_type = g:clap.provider._().source_type
    else
      let g:clap.provider.source_type = s:detect_source_type()
      let g:clap.registrar[provider_id]['source_type'] = g:clap.provider.source_type
    endif
  endif

  call clap#selection#init()

  silent doautocmd <nomodeline> User ClapOnInitialize

  " This flag is used to slience the autocmd events for NeoVim, e.g., on_typed.
  " Vim doesn't have these issues as it uses noautocmd in most cases.
  "
  " Without this flag, the on_typed hook can be triggered when relaunching
  " some provider. To reproduce:
  " 1. :Clap
  " 2. Choose proj_tags
  " 3. proj_tags ontyped hook will be triggered.
  let g:__clap_open_win_pre = v:true
  call g:clap.open_win()
  let g:__clap_open_win_pre = v:false
endfunction

if !exists('g:clap')
  call clap#init#()
endif

function! s:parse_opts(args) abort
  let idx = 0
  let g:clap.provider.args = []
  for arg in a:args
    if arg ==# '--'
      let g:clap.context.query = join(a:args[idx+1 :], ' ')
      break
    endif
    if arg =~? '^++\w*=\w*'
      let matched = matchlist(arg, '^++\(\w*\)=\(\S*\)')
      let [k, v] = [matched[1], matched[2]]
      if has_key(g:clap.context, k)
        let g:clap.context[k] .= ' '.v
      else
        let g:clap.context[k] = v
      endif
    elseif arg =~? '^+\w*'
      let opt = arg[1:]
      let g:clap.context[opt] = v:true
    else
      call add(g:clap.provider.args, arg)
    endif
    let idx += 1
  endfor
  if has_key(g:clap.context, 'query')
    if g:clap.context.query ==# '@visual'
      let g:clap.context.query = clap#util#get_visual_selection()
    else
      let g:clap.context.query = clap#util#expand(g:clap.context.query)
    endif
  endif
endfunction

function! clap#(bang, ...) abort
  if a:000 == ['install-binary']
    call clap#installer#install(v:false)
    return
  elseif a:000 == ['install-binary!']
    call clap#installer#install(v:true)
    return
  endif

  let g:clap.start.bufnr = bufnr('')
  let g:clap.start.winid = win_getid()
  let g:clap.start.old_pos = getpos('.')

  let g:clap.context = {'visible': v:false}
  let g:clap.tmps = []

  if a:bang
    let g:clap.context.async = v:true
  endif

  if a:0 == 0
    let provider_id_or_alias = 'providers'
    let g:clap.provider.args = []
  else
    if a:000 == ['debug']
      call clap#debugging#info()
      return
    elseif a:000 == ['debug+']
      call clap#debugging#info_to_clipboard()
      return
    endif
    if a:1 ==# '!'
      let g:clap.context['no-cache'] = v:true
      let provider_id_or_alias = a:2
      call s:parse_opts(a:000[2:])
    else
      let provider_id_or_alias = a:1
      call s:parse_opts(a:000[1:])
    endif
  endif

  if provider_id_or_alias =~# '!$'
    let g:clap.context['no-cache'] = v:true
    let provider_id_or_alias = provider_id_or_alias[:-2]
  endif

  call clap#for(provider_id_or_alias)
endfunction

function! clap#run(provider) abort
  let id = has_key(a:provider, 'id') ? a:provider['id'] : 'run'
  let g:clap_provider_{id} = a:provider
  if s:inject_default_impl_is_ok(g:clap_provider_{id})
        \ && s:validate_provider(g:clap_provider_{id})
    let g:clap.registrar[id] = g:clap_provider_{id}
    execute 'Clap' id
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
