" No usual did_ftplugin_loaded check

let s:use_gui = has('gui_running') || (has('termguicolors') && &termguicolors)
let s:gui_or_cterm = s:use_gui ? 'gui' : 'cterm'

function! s:get_color(group, attr) abort
  return synIDattr(synIDtrans(hlID(a:group)), a:attr)
endfunction

function! s:get_attrs(group) abort
  let fg = s:get_color(a:group, 'fg')
  if empty(fg)
    let fg = s:normal_fg
  endif
  return printf('%sbg=%s %sfg=%s', s:gui_or_cterm, s:normal_bg, s:gui_or_cterm, fg)
endfunction

let s:normal_fg = s:get_color('Normal', 'fg')
if empty(s:normal_fg)
  let s:normal_fg = s:gui_or_cterm ==# 'gui' ? '#b2b2b2' : 249
endif

let s:normal_bg = s:get_color('Normal', 'bg')
if empty(s:normal_bg)
  let s:normal_bg = s:gui_or_cterm ==# 'gui' ? '#292b2e' : 235
endif

if !exists('s:hi_icon')
  let icons = clap#icon#get_all()
  let hi_groups = [
        \ 'ModeMsg',
        \ 'Type',
        \ 'Number',
        \ 'Float',
        \ 'CursorLineNr',
        \ 'Question',
        \ 'Title',
        \ 'Cursor',
        \ 'VisualNC',
        \ 'WildMenu',
        \ 'Folded',
        \ 'FoldColumn',
        \ 'DiffAdd',
        \ 'DiffChange',
        \ 'DiffText',
        \ 'SignColumn',
        \ 'TabLine',
        \ ]
  let hi_idx = 0
  let hi_groups_len = len(hi_groups)
  let s:groups = []
  for idx in range(len(icons))
    let group = 'ClapIcon'.idx
    call add(s:groups, group)
    let icon = icons[idx]
    execute 'syntax match' group '/'.icon.'/' 'contained'
    " execute 'highlight default link' group hi_groups[hi_idx]
    execute 'hi!' group s:get_attrs(hi_groups[hi_idx])
    let hi_idx += 1
    let hi_idx = hi_idx % hi_groups_len
  endfor
  let s:hi_icon = 1
endif

syntax match ClapLinNr /^.*:\zs\d\+\ze:\d\+:/hs=s+1,he=e-1
syntax match ClapColumn /:\d\+:\zs\d\+\ze:/ contains=ClapLinNr
syntax match ClapLinNrColumn /\zs:\d\+:\d\+:\ze/ contains=ClapLinNr,ClapColumn

execute 'syntax match ClapFpath' '/^.*:\d\+:\d\+:/' 'contains=ClapLinNrColumn,'.join(s:groups, ',')

hi default link ClapFpath            Keyword
hi default link ClapLinNr            LineNr
hi default link ClapColumn           Comment
hi default link ClapLinNrColumn      Type
