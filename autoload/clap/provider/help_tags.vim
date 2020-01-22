" Author: Mark Wu <markplace@gmail.com>
" Description: List the help tags, ported from https://github.com/zeero/vim-ctrlp-help

let s:save_cpo = &cpoptions
set cpoptions&vim

let s:help_tags_memory_cache = []

function! s:help_tags_source() abort
  let help_tags_cache_file = clap#cache#location_for('help_tags', 'help_tags.txt')
  if empty(s:help_tags_memory_cache)
    if getftime(help_tags_cache_file) > max(map(s:get_tags_files(), 'getftime(v:val)'))
      if filereadable(help_tags_cache_file)
        let s:help_tags_memory_cache = readfile(help_tags_cache_file)
      endif
    else
      let s:help_tags_memory_cache = s:get_tags_list()
      silent! call writefile(s:help_tags_memory_cache, help_tags_cache_file)
    endif
  endif

  return s:help_tags_memory_cache
endfunction

function! s:get_tags_list() abort
  let tagsfiles = s:get_tags_files()

  let input_dict = {}
  for tagsfile in tagsfiles
    for line in readfile(tagsfile)
      let items = split(line, "\t")
      let tag_subject = items[0]
      if !has_key(input_dict, tag_subject)
        let input_dict[tag_subject] = printf("%-60s\t%s", tag_subject, items[1])
      endif
    endfor
  endfor
  let input = sort(values(input_dict))

  return input
endfunction

function! s:get_tags_files() abort
  let tagspaths = map(filter(split(&helplang, ','), 'v:val !=? "en"'), '"/doc/tags-".v:val')
  call add(tagspaths, '/doc/tags')

  let tagsfiles = []
  for tagspath in tagspaths
    call extend(tagsfiles, filter(map(split(&runtimepath, ','), 'v:val . tagspath'), 'filereadable(v:val)'))
  endfor

  return tagsfiles
endfunction

function! s:help_tags_sink(line) abort
  let tag = get(split(a:line, "\t"), 0)
  execute 'help' tag
endfunction

let s:help_tags = {}
let s:help_tags.sink = function('s:help_tags_sink')
let s:help_tags.source = function('s:help_tags_source')

let g:clap#provider#help_tags# = s:help_tags

let &cpoptions = s:save_cpo
unlet s:save_cpo
