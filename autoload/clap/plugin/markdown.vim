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

" Called when preview is updated with a new file
function! clap#plugin#markdown#on_preview_updated(info) abort
  " Show notification to remind user to switch to browser
  call clap#helper#echo_info('Markdown preview updated - switch to browser to view')
endfunction
