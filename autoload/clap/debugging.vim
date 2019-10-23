" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Gather some info useful for debugging.

let s:save_cpo = &cpoptions
set cpoptions&vim

function! s:get_global_variables() abort
  let variable_list = []

  for key in keys(g:)
    if key ==# 'clap'
      continue
    endif
    if key =~# '^clap'
      call add(variable_list, key)
    endif
  endfor

  " Ignore the variables of builtin variables
  call filter(variable_list, 'v:val !~# ''^clap#provider#''')
  call filter(variable_list, 'v:val !~# ''^clap#icon#''')
  call filter(variable_list, 'v:val !~# ''clap#floating_win#''')
  call filter(variable_list, 'v:val !~# ''clap#builtin_providers''')

  call sort(variable_list)

  return variable_list
endfunction

function! s:get_third_party_providers() abort
  let all_providers = split(globpath(&rtp, 'autoload/clap/provider/*.vim'), "\n")
  let third_party_providers = filter(all_providers, 'index(g:clap#builtin_providers, v:val) != -1')
  return third_party_providers
endfunction

function! clap#debugging#info() abort
  let third_party_providers = string(s:get_third_party_providers())
  let global_variables = s:get_global_variables()
  echohl Type   | echo '     Current FileType: ' | echohl NONE
  echohl Normal | echon &filetype                | echohl NONE
  echohl Type   | echo 'Third Party Providers: ' | echohl NONE
  echohl Normal | echon third_party_providers    | echohl NONE
  echohl Type   | echo '     Global Variables:'  | echohl NONE
  echo '    let g:clap.registrar = '.string(g:clap.registrar)
  for variable in global_variables
    echo '    let g:'.variable.' = '. string(g:[variable])
  endfor
endfunction

function! clap#debugging#info_to_clipboard() abort
  redir => l:output
    silent call clap#debugging#info()
  redir END

  let @+ = l:output
  echohl Type     | echo '[vim-clap] '                | echohl NONE
  echohl Function | echon 'Vim-clap info'             | echohl NONE
  echohl Normal   | echon ' copied to your clipboard' | echohl NONE
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
