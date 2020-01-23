let s:is_nvim = has('nvim')

let s:input_default_hi_group = 'Visual'
let s:display_default_hi_group = 'Pmenu'
let s:preview_defaualt_hi_group = 'PmenuSel'

function! s:extract(group, what, gui_or_cterm) abort
  return synIDattr(synIDtrans(hlID(a:group)), a:what, a:gui_or_cterm)
endfunction

function! s:extract_or(group, what, gui_or_cterm, default) abort
  let v = s:extract(a:group, a:what, a:gui_or_cterm)
  return empty(v) ? a:default : v
endfunction

function! s:hi_display_invisible() abort
  " People can use their own display highlight group, so can't use s:display_default_hi_group here.
  let guibg = s:extract_or(s:display_group, 'bg', 'gui', '#544a65')
  let ctermbg = s:extract_or(s:display_group, 'bg', 'cterm', '60')
  execute printf(
        \ 'hi ClapDisplayInvisibleEndOfBuffer ctermfg=%s guifg=%s',
        \ ctermbg,
        \ guibg
        \ )
endfunction

function! s:hi_preview_invisible() abort
  let guibg = s:extract_or(s:preview_group, 'bg', 'gui', '#5e5079')
  let ctermbg = s:extract_or(s:preview_group, 'bg', 'cterm', '60')
  execute printf(
        \ 'hi ClapPreviewInvisibleEndOfBuffer ctermfg=%s guifg=%s',
        \ ctermbg,
        \ guibg
        \ )
endfunction

" Try to sync the spinner bg with input window.
function! s:hi_spinner() abort
  let vis_ctermbg = s:extract_or(s:input_default_hi_group, 'bg', 'cterm', '60')
  let vis_guibg = s:extract_or(s:input_default_hi_group, 'bg', 'gui', '#544a65')
  let fn_ctermfg = s:extract_or('Function', 'fg', 'cterm', '170')
  let fn_guifg = s:extract_or('Function', 'fg', 'gui', '#bc6ec5')

  execute printf(
        \ 'hi ClapSpinner guifg=%s ctermfg=%s ctermbg=%s guibg=%s gui=bold cterm=bold',
        \ fn_guifg,
        \ fn_ctermfg,
        \ vis_ctermbg,
        \ vis_guibg,
        \ )
endfunction

function! s:hi_clap_symbol() abort
  let input_ctermbg = s:extract_or('ClapInput', 'bg', 'cterm', '60')
  let input_guibg = s:extract_or('ClapInput', 'bg', 'gui', '#544a65')
  let normal_ctermfg = s:extract_or('Normal', 'bg', 'cterm', '249')
  let normal_guifg = s:extract_or('Normal', 'bg', 'gui', '#b2b2b2')
  execute printf(
        \ 'hi ClapSymbol guifg=%s ctermfg=%s ctermbg=%s guibg=%s',
        \ input_guibg,
        \ input_ctermbg,
        \ normal_ctermfg,
        \ normal_guifg,
        \ )
endfunction

function! s:colorschme_adaptive() abort
  call s:hi_display_invisible()
  call s:hi_preview_invisible()
  call s:hi_clap_symbol()
  call clap#icon#def_color_components()
endfunction

function! s:highlight_for(group_name, props) abort
  execute 'hi' a:group_name join(values(map(copy(a:props), 'v:key."=".v:val')), ' ')
endfunction

function! s:try_apply_themes_is_ok(theme_name) abort
  try
    let palette = g:clap#themes#{a:theme_name}#palette
    call s:highlight_for('ClapSpinner', palette.spinner)
    call s:highlight_for('ClapInput', palette.input)
    call s:highlight_for('ClapDisplay', palette.display)
    call s:highlight_for('ClapSelected', palette.selected)
    call s:highlight_for('ClapCurrentSelection', palette.current_selection)
    " Backward compatible
    if hlexists('ClapQuery')
      hi link ClapSearchText ClapQuery
    else
      call s:highlight_for('ClapSearchText', palette.search_text)
    endif
  catch
    return v:false
  endtry
  return v:true
endfunction

function! s:apply_default_theme() abort
  " if !hlexists('ClapSpinner')
    " call s:hi_spinner()
    " augroup ClapRefreshSpinner
      " autocmd!
      " autocmd ColorScheme * call s:hi_spinner()
    " augroup END
  " endif

  if !hlexists('ClapQuery')
    " A bit repeatation code here in case of ClapSpinner is defined explicitly.
    let vis_ctermbg = s:extract_or(s:input_default_hi_group, 'bg', 'cterm', '60')
    let vis_guibg = s:extract_or(s:input_default_hi_group, 'bg', 'gui', '#544a65')
    let ident_ctermfg = s:extract_or('Normal', 'fg', 'cterm', '249')
    let ident_guifg = s:extract_or('Normal', 'fg', 'gui', '#b2b2b2')
    execute printf(
          \ 'hi ClapQuery guifg=%s ctermfg=%s ctermbg=%s guibg=%s cterm=bold gui=bold',
          \ ident_guifg,
          \ ident_ctermfg,
          \ vis_ctermbg,
          \ vis_guibg,
          \ )
  endif

  hi ClapDefaultPreview          ctermbg=237 guibg=#3E4452
  hi ClapDefaultSelected         ctermfg=80  guifg=#5fd7d7 cterm=bold,underline gui=bold,underline
  hi ClapDefaultCurrentSelection ctermfg=224 guifg=#ffd7d7 cterm=bold gui=bold

  hi default link ClapPreview ClapDefaultPreview
  hi default link ClapSelected ClapDefaultSelected
  hi default link ClapCurrentSelection ClapDefaultCurrentSelection

  call s:hi_clap_symbol()

  let s:display_group = hlexists('ClapDisplay') ? 'ClapDisplay' : s:display_default_hi_group
  call s:hi_display_invisible()

  let s:preview_group = hlexists('ClapPreview') ? 'ClapPreview' : 'ClapDefaultPreview'
  call s:hi_preview_invisible()

  augroup ClapColorSchemeAdaptive
    autocmd!
    autocmd ColorScheme * call s:colorschme_adaptive()
  augroup END

  execute 'hi default link ClapInput' s:input_default_hi_group
  execute 'hi default link ClapDisplay' s:display_default_hi_group
endfunction

function! clap#themes#init_hi_groups() abort
  hi default link ClapMatches Search
  hi default link ClapNoMatchesFound ErrorMsg
  hi default link ClapPopupCursor Type

  if !s:try_apply_themes_is_ok('material_design_dark')
    call s:apply_default_theme()
  endif
endfunction
