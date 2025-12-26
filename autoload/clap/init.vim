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
  " Softer submatch colors for better visual harmony
  let clap_sub_matches = [
        \ [180 , '#fab387'] ,
        \ [204 , '#f38ba8'] ,
        \ [186 , '#f9e2af'] ,
        \ [75  , '#7dcfff'] ,
        \ [176 , '#cba6f7'] ,
        \ [180 , '#f5c2e7'] ,
        \ [143 , '#a6e3a1'] ,
        \ [73  , '#94e2d5'] ,
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
    " Softer, more pleasant color gradient inspired by Tokyo Night
    " Uses blues, cyans, and soft greens for better readability
    let clap_fuzzy_matches = [
          \ [75  , '#7dcfff'] ,
          \ [81  , '#7aa2f7'] ,
          \ [110 , '#89b4fa'] ,
          \ [114 , '#9ece6a'] ,
          \ [150 , '#a6e3a1'] ,
          \ [116 , '#94e2d5'] ,
          \ [152 , '#b4befe'] ,
          \ [183 , '#cba6f7'] ,
          \ [218 , '#f5c2e7'] ,
          \ ]
  endif

  " idx from 1
  call map(clap_fuzzy_matches, 's:add_fuzzy_match_hl_group(v:key+1, v:val[0], v:val[1])')

  let g:__clap_fuzzy_matches_hl_group_cnt = len(clap_fuzzy_matches)
  let g:__clap_fuzzy_last_hl_group = 'ClapFuzzyMatches'.g:__clap_fuzzy_matches_hl_group_cnt
endfunction

function! clap#init#() abort
  call clap#api#clap#init()
  call clap#themes#init()

  call s:init_submatches_hl_group()
  call s:init_fuzzy_match_hl_groups()

  " Spawn the daemon process if not running
  if !clap#job#daemon#is_running()
    call clap#job#daemon#start()
  endif
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
