" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Primiary code path for the plugin.

let s:save_cpo = &cpo
set cpo&vim

if has('nvim')
  let s:has_features = has('nvim-0.4')
else
  let s:has_features = has('patch-8.1.1967')
endif

if !s:has_features
  echoerr 'Vim-clap requires NeoVim >= 0.4.0 or Vim 8.1.1967+'
  echoerr 'Please upgrade your editor accordingly.'
  finish
endif

let s:cur_dir = fnamemodify(resolve(expand('<sfile>:p')), ':h')
let s:builtin_providers = map(
      \ split(globpath(s:cur_dir.'/clap/provider', '*'), '\n'),
      \ 'fnamemodify(v:val, '':t:r'')'
      \ )

let g:clap#builtin_providers = s:builtin_providers

let s:provider_alias = {
      \ 'hist:': 'command_history',
      \ 'gfiles': 'git_files',
      \ }

let s:provider_alias = extend(s:provider_alias, get(g:, 'clap_provider_alias', {}))

let s:default_action = {
  \ 'ctrl-t': 'tab split',
  \ 'ctrl-x': 'split',
  \ 'ctrl-v': 'vsplit',
  \ }

let g:clap_no_matches_msg = get(g:, 'clap_no_matches_msg', 'NO MATCHES FOUND')
let g:__clap_no_matches_pattern = '^'.g:clap_no_matches_msg.'$'

function! clap#action_for(action) abort
  return get(s:default_action, a:action, '')
endfunction

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
  echom "_ unimplemented"
endfunction

function! clap#exit() abort
  " NOTE: Need to go back to the start window
  call g:clap.start.goto_win()

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

  call clap#sign#reset()

  silent doautocmd <nomodeline> User ClapOnExit
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
      call clap#error("Fail to load the provider: ".provider_id)
      return v:false
    endtry
  endif

  if !s:inject_default_impl_is_ok(registration_info)
    return v:false
  endif

  let g:clap.registrar[provider_id] = {}
  call extend(g:clap.registrar[provider_id], registration_info)

  if has_key(registration_info, 'alias')
    let s:alias_cache[registration_info.alias] = provider_id
  endif

  return v:true
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

if !exists('g:clap')
  call clap#init#()
  call clap#register('_', {'source': s:builtin_providers, 'sink': function('s:_sink')})
endif

function! clap#(bang, ...) abort
  let g:clap.start.bufnr = bufnr('')
  let g:clap.start.winid = win_getid()
  let g:clap.start.old_pos = getpos('.')

  if a:0 == 0
    let provider_id_or_alias = '_'
  else
    if a:000 == ['debug']
      call clap#debugging#info()
      return
    elseif a:000 == ['debug+']
      call clap#debugging#info_to_clipboard()
      return
    endif
    let provider_id_or_alias = a:1
    let args = a:000[1:]
    let g:clap.provider.args = clap#util#expand(args)
  endif

  call clap#for(provider_id_or_alias)
endfunction

let &cpo = s:save_cpo
unlet s:save_cpo
