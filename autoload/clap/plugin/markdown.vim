" autoload/clap/plugin/markdown.vim

" Called when the browser preview tab/window is closed
function! clap#plugin#markdown#on_browser_closed(info) abort
  let l:bufnr = get(a:info, 'bufnr', -1)

  if l:bufnr == -1
    return
  endif

  " Show notification to user
  call clap#helper#echo_info('Markdown preview browser was closed')
endfunction
