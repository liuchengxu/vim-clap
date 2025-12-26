" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Initialize the plugin, including making a compatible API layer
" and flexiable highlight groups.

let s:save_cpo = &cpoptions
set cpoptions&vim

function! s:define_highlight_group(group_name, cermfg, guifg) abort
  if !hlexists(a:group_name)
    execute printf(
          \ 'hi %s ctermfg=%s guifg=%s ctermbg=%s guibg=%s gui=bold cterm=bold',
          \ a:group_name,
          \ a:cermfg,
          \ a:guifg,
          \ 'NONE',
          \ 'NONE',
          \ )
  endif
endfunction

function! s:init_submatches_hl_group() abort
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

  " idx from 1
  call map(clap_sub_matches, 's:define_highlight_group("ClapMatches".(v:key+1), v:val[0], v:val[1])')
endfunction

function! s:add_fuzzy_match_hl_group(idx, ctermfg, guifg) abort
  let group_name = 'ClapFuzzyMatches'.a:idx
  call s:define_highlight_group(group_name, a:ctermfg, a:guifg)
  if !has('nvim')
    call prop_type_add(group_name, {'highlight': group_name})
  endif
endfunction

function! s:init_fuzzy_match_hl_groups() abort
  if exists('g:clap_fuzzy_match_hl_groups')
    let clap_fuzzy_matches = g:clap_fuzzy_match_hl_groups
  else
    let clap_fuzzy_matches = [
          \ [118 , '#87ff00'] ,
          \ [82  , '#5fff00'] ,
          \ [46  , '#00ff00'] ,
          \ [47  , '#00ff5f'] ,
          \ [48  , '#00ff87'] ,
          \ [49  , '#00ffaf'] ,
          \ [50  , '#00ffd7'] ,
          \ [51  , '#00ffff'] ,
          \ [87  , '#5fffff'] ,
          \ ]
  endif

  " idx from 1
  call map(clap_fuzzy_matches, 's:add_fuzzy_match_hl_group(v:key+1, v:val[0], v:val[1])')

  let g:__clap_fuzzy_matches_hl_group_cnt = len(clap_fuzzy_matches)
  let g:__clap_fuzzy_last_hl_group = 'ClapFuzzyMatches'.g:__clap_fuzzy_matches_hl_group_cnt
endfunction

function! s:init_path_highlight_groups() abort
  " For Vim, we need to define text property types for path highlighting
  if !has('nvim')
    try
      call prop_type_add('ClapPathPrefix', {'highlight': 'ClapPathPrefix'})
      call prop_type_add('ClapFileName', {'highlight': 'ClapFileName'})
    catch
      " Types may already exist
    endtry
  endif
endfunction

function! clap#init#() abort
  call clap#api#clap#init()
  call clap#themes#init()

  call s:init_submatches_hl_group()
  call s:init_fuzzy_match_hl_groups()
  call s:init_path_highlight_groups()

  " Spawn the daemon process if not running
  if !clap#job#daemon#is_running()
    call clap#job#daemon#start()
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
