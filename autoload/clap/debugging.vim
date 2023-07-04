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
  call filter(variable_list, 'v:val !~# ''clap#floating_win''')
  call filter(variable_list, 'v:val !~# ''clap#display_win''')
  call filter(variable_list, 'v:val !~# ''clap#popup''')
  call filter(variable_list, 'v:val !~# ''clap#themes#''')

  call sort(variable_list)

  return variable_list
endfunction

function! s:get_third_party_providers() abort
  let all_providers = split(globpath(&runtimepath, 'autoload/clap/provider/*.vim'), "\n")
  let third_party_providers = filter(all_providers, 'index(clap#builtin_providers(), v:val) != -1')
  return third_party_providers
endfunction

function! clap#debugging#info() abort
  let third_party_providers = string(s:get_third_party_providers())
  let global_variables = s:get_global_variables()

  if has('nvim')
    echohl Type   | echo '               NeoVim: '                  | echohl NONE
    echohl Normal | echon split(execute('version'), '\n')[0]     | echohl NONE
  else
    echohl Type   | echo '                  Vim: '                            | echohl NONE
    echohl Normal | echon join(split(execute('version'), '\n')[:1])     | echohl NONE
  endif

  echohl Type   | echo '            has ctags: '                | echohl NONE
  if executable('ctags')
    let ctags_version = split(split(system('ctags --version'), '\n')[0], ',')[0]
    let support_json_format = !empty(filter(systemlist('ctags --list-features'), 'v:val =~# ''^json'''))
    let json_support = support_json_format ? ' (+json)' : ' (-json)'
    echohl Normal | echon ctags_version.json_support    | echohl NONE
  else
    echohl Normal | echon 'ctags not found'    | echohl NONE
  endif

  echohl Type   | echo '            has cargo: ' | echohl NONE
  echohl Normal | echon executable('cargo')      | echohl NONE

  let maple_binary = clap#maple#binary()
  if maple_binary is v:null
    echohl Type   | echo '            has maple: 0' | echohl NONE
  else
    echohl Type   | echo '            has maple: ' | echohl NONE
    echohl Normal | echon maple_binary             | echohl NONE

    echohl Type | echo '           maple info: ' | echohl NONE
    " Note: maple_binary has to be quoted, otherwise error happens when the path contains spaces.
    let maple_version = system(printf('"%s" version', maple_binary))
    if v:shell_error
      echohl Normal | echon '[ERROR]fail to fetch version info' | echohl NONE
    else
      echohl Normal | echon maple_version | echohl NONE
    endif
  endif

  if executable('rustc')
    echohl Type   | echo '        rustc version: '  | echohl NONE
    echohl Normal | echon system('rustc --version') | echohl NONE
  endif

  echohl Type   | echo '     Current FileType: ' | echohl NONE
  echohl Normal | echon &filetype                | echohl NONE

  echohl Type   | echo 'Third Party Providers: ' | echohl NONE
  echohl Normal | echon third_party_providers    | echohl NONE

  echohl Type   | echo '       Global Options:'  | echohl NONE
  let provider_var = []
  for variable in global_variables
    if variable =~# 'clap_provider_'
      call add(provider_var, variable)
    else
      echo '    let g:'.variable.' = '. string(g:[variable])
    endif
  endfor

  echohl Type   | echo '  Provider Variables:'  | echohl NONE
  if empty(provider_var)
    echo '                     []'
  else
    for variable in provider_var
      echo '    let g:'.variable.' = '. string(g:[variable])
    endfor
  endif
endfunction

function! clap#debugging#info_to_clipboard() abort
  redir => l:output
    silent call clap#debugging#info()
  redir END

  let @+ = l:output
  echohl Type     | echo '[vim-clap] '                | echohl NONE
  echohl Function | echon 'Clap debug info'           | echohl NONE
  echohl Normal   | echon ' copied to your clipboard' | echohl NONE
endfunction

let &cpoptions = s:save_cpo
unlet s:save_cpo
