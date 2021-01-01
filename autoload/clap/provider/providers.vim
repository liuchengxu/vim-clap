" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: List all the providers.

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:providers = {}

function! s:providers.sink(selected) abort
  let provider = a:selected[: stridx(a:selected, ':') - 1]
  " a sink for "Clap providers" (dispatch to other builtin clap providers).
  call timer_start(0, {-> clap#_for(provider)})
endfunction

function! s:providers.source() abort
  if !exists('s:global_source')
    let s:global_source = []

    for autoload_provider in split(globpath(&runtimepath, 'autoload/clap/provider/*.vim'), "\n")
      let provider_id = fnamemodify(autoload_provider, ':t:r')
      if file_readable(autoload_provider)
        let desc_line = readfile(autoload_provider, '', 2)[-1]
        let desc = matchstr(desc_line, '^.*Description: \zs\(.*\)\ze\.\?$')
        if empty(desc)
          call add(s:global_source, provider_id.':')
        else
          call add(s:global_source, provider_id.': '.desc)
        endif
      else
        call add(s:global_source, provider_id.':')
      endif
    endfor

    " `description` is required, otherwise we can't distinguish whether the variable name
    " like `g:clap_provider_yanks_history` is a name of some provider or merely a control
    " variable of a provider.
    let maybe_user_var_providers = filter(keys(g:), 'v:val =~# "^clap_provider_"')
    for maybe_var_provider in maybe_user_var_providers
      try
        let evaled = eval('g:'.maybe_var_provider)
        if type(evaled) == v:t_dict
          let provider_id = matchstr(maybe_var_provider, 'clap_provider_\zs\(.*\)')
          call add(s:global_source, provider_id.': '.evaled['description'])
        endif
      catch
        " Ignore
      endtry
    endfor
  endif
  return s:global_source
endfunction

let s:providers.syntax = 'clap_providers'
let g:clap#provider#providers# = s:providers

let &cpoptions = s:save_cpo
unlet s:save_cpo
