scriptencoding utf-8

if !exists('s:groups')
  let s:groups = ['Character', 'Float', 'Identifier', 'Statement', 'Label', 'Boolean', 'Delimiter', 'Constant', 'String', 'Operator', 'PreCondit', 'Include', 'Conditional', 'PreProc', 'TypeDef',]
  let s:len_groups = len(s:groups)
endif

function! s:hi() abort
  let icons = clap#icon#get_all()

  let idx = 0
  let hi_idx = 0

  let icon_groups = []
  for icon in icons
    let cur_group = 'ClapVistaIcon'.idx
    call add(icon_groups, cur_group)
    execute 'syntax match' cur_group '/^\s*'.icon.'/' 'contained'
    execute 'hi default link' cur_group s:groups[hi_idx]
    let hi_idx += 1
    let hi_idx = hi_idx % s:len_groups
    let idx += 1
  endfor

  let joined_icon_groups = join(icon_groups, ',')

  execute 'syntax match ClapVistaTag    /\s*.*\(:\d\)\@=/' 'contains=ClapVistaIcon,'.joined_icon_groups
  execute 'syntax match ClapVistaNumber /^[^\[]*\(\s\s\[\)\@=/' 'contains=ClapVistaTag,ClapVistaIcon,'.joined_icon_groups
  syntax match ClapVistaScope  /^[^]]*]/ contains=ClapVistaNumber,ClapVistaBracket
  syntax match ClapVista /^[^│┌└]*/ contains=ClapVistaBracket,ClapVistaTag,ClapVistaNumber,ClapVistaScope
  syntax match ClapVistaBracket /\s\s\[\|\]\s\s/ contained

  hi default link ClapVistaBracket  SpecialKey
  hi default link ClapVistaNumber   Number
  hi default link ClapVistaTag      Tag
  hi default link ClapVistaScope    Function
  hi default link ClapVista         Type
endfunction

call s:hi()
