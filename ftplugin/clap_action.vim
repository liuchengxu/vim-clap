nnoremap <silent> <buffer> <CR>      :<c-u>call clap#floating_win#action#apply_choice()<CR>
nnoremap <silent> <buffer> <Esc>     :<c-u>call clap#floating_win#action#close()<CR>
nnoremap <silent> <buffer> q         :<c-u>call clap#floating_win#action#close()<CR>
nnoremap <silent> <buffer> j         :<c-u>call clap#floating_win#action#next_item()<CR>
nnoremap <silent> <buffer> k         :<c-u>call clap#floating_win#action#prev_item()<CR>
