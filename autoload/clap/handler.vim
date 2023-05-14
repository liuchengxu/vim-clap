" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Handle the movement.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:is_nvim = has('nvim')
let s:old_input = ''

function! clap#handler#relaunch_providers() abort
  call clap#handler#exit()
  call timer_start(10, { -> clap#for('providers') })
  call g:clap.input.set('')
endfunction

function! clap#handler#relaunch_is_ok(cur_input) abort
  if a:cur_input ==# g:clap_providers_relaunch_code
    call clap#handler#relaunch_providers()
    return v:true
  endif
  return v:false
endfunction

function! clap#handler#on_typed() abort
  " CursorMoved event can be triggered when the floating_win
  " has been created but not visible yet.
  if s:is_nvim && !g:clap.context.visible
    return
  endif

  let l:cur_input = g:clap.input.get()

  if clap#handler#relaunch_is_ok(l:cur_input)
    return
  endif

  " This check is necessary for now, but it might be removed if the underlying
  " logic can be revisited why removing it breaks the sink behavior.
  if s:old_input == l:cur_input
    return
  endif
  let s:old_input = l:cur_input

  call g:clap.provider.on_typed()
endfunction

function! s:handle_no_matches() abort
  if has_key(g:clap.provider._(), 'on_no_matches')
    let input = g:clap.input.get()
    call s:internal_exit()
    call g:clap.provider._().on_no_matches(input)
    call g:clap.provider.on_exit()
    silent doautocmd <nomodeline> User ClapOnExit
  else
    call clap#handler#exit()
  endif
endfunction

function! s:provider_sink() abort
  " This could be more robust by checking the exact matches count, but this should also be enough.
  if empty(g:clap.display.getcurline())
        \ || g:clap.display.get_lines() == [g:clap_no_matches_msg]
    call s:handle_no_matches()
    return
  endif

  let [Sink, sink_args] = clap#selection#get_sink_or_sink_star_params()

  let preserved_selections = clap#sign#preserved_selections()

  call s:internal_exit()

  try
    " Handle the preserved selections specially.
    "
    " Typically, if `Sink` is sink_star, `sink_args` is a List of String,
    " if `Sink` is sink, `sink_args` is a String. Due to the support requested
    " in https://github.com/liuchengxu/vim-clap/issues/737, `sink_args` can be
    " multiple items for sink with the preserved selections considered.
    if !empty(preserved_selections) && g:clap.provider.id ==# 'files'
      let Sink = g:clap.provider.sink_star
      let sink_args = type(sink_args) == v:t_list ? extend(preserved_selections, sink_args) : add(preserved_selections, sink_args)
    endif

    call Sink(sink_args)
  catch
    call clap#helper#echo_error('s:provider_sink: '.v:exception.', throwpoint:'.v:throwpoint)
  finally
    call g:clap.provider.on_exit()
    silent doautocmd <nomodeline> User ClapOnExit
  endtry
endfunction

" Similiar to s:provider_sink() but using a custom Sink function and without
" handling the no matches case.
function! clap#handler#sink_with(Fn, ...) abort
  call s:internal_exit()
  try
    call call(a:Fn, a:000)
  catch
    call clap#helper#echo_error('clap#handler#sink_with: '.v:exception.', throwpoint:'.v:throwpoint)
  finally
    call g:clap.provider.on_exit()
    silent doautocmd <nomodeline> User ClapOnExit
  endtry
endfunction

function! s:internal_exit() abort
  call clap#selection#reset()
  call clap#exit()
endfunction

" clap#handler#exit() = s:internal_exit() + external on_exit hook
function! clap#handler#exit() abort
  call s:internal_exit()
  call g:clap.provider.on_exit()
  silent doautocmd <nomodeline> User ClapOnExit
  let s:old_input = ''
endfunction

function! s:noop() abort
endfunction

" Neovim only, BS handler of vim is in popup/move_manager.vim
function! s:on_backspace() abort
  call nvim_feedkeys("\<BS>", 'n', v:true)
endfunction

let s:default_mappings = {
      \ "<BS>": function('s:on_backspace'),
      \ "<CR>": function('s:provider_sink'),
      \ "<Tab>": function('clap#selection#toggle'),
      \ "<A-U>": function('s:noop'),
      \ }

function! clap#handler#handle_mapping(mapping) abort
  if has_key(get(g:clap.provider._(), 'mappings', {}), a:mapping)
    call g:clap.provider._().mappings[a:mapping]()
  else
    call s:default_mappings[a:mapping]()
  endif
  return ''
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
