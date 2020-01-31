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
    for provider_id in g:clap#builtin_providers
      let provider_path = globpath(&runtimepath, 'autoload/clap/provider/'.provider_id.'.vim')
      if file_readable(provider_path)
        let desc_line = readfile(provider_path, '', 2)[-1]
        let desc = matchstr(desc_line, '^.*Description: \zs\(.*\)\ze\.\?$')
        if empty(desc)
          call add(s:global_source, provider_id.':')
        else
          call add(s:global_source, provider_id.': '.desc)
        endif
      endif
    endfor
  endif
  return s:global_source
endfunction

let s:providers.syntax = 'clap_providers'
let g:clap#provider#providers# = s:providers

let &cpoptions = s:save_cpo
unlet s:save_cpo
