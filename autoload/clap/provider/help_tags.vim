" Author: Mark Wu <markplace@gmail.com>
" Description: List the help tags, ported from https://github.com/zeero/vim-ctrlp-help

let s:save_cpo = &cpoptions
set cpoptions&vim

function! s:get_doc_tags() abort
  return ['/doc/tags'] + map(filter(split(&helplang, ','), 'v:val !=? "en"'), '"/doc/tags-".v:val')
endfunction

if clap#maple#is_available() && clap#filter#python#has_dynamic_module()

  function! s:help_tags_source() abort
    let tmp = tempname()
    call writefile([join(s:get_doc_tags(), ','), &runtimepath], tmp)
    return clap#maple#inject_bin('helptags '.tmp)
  endfunction

  function! s:help_tags_sink(line) abort
    let [tag, doc_fname] = split(a:line, "\t")
    if doc_fname =~# '.txt$'
      execute 'help' trim(tag).'@en'
    else
      execute 'help' tag
    endif
  endfunction

else

  let s:help_tags_memory_cache = []

  function! s:help_tags_source() abort
    if empty(s:help_tags_memory_cache)
      let help_tags_cache_file = clap#cache#location_for('help_tags', 'help_tags.txt')
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
    let tags_files = s:get_tags_files()

    let tags_dict = {}
    for tagsfile in tags_files
      for line in readfile(tagsfile)
        let items = split(line, "\t")
        let tag_subject = items[0]
        if !has_key(tags_dict, tag_subject)
          let tags_dict[tag_subject] = printf("%-60s\t%s", tag_subject, items[1])
        endif
      endfor
    endfor

    return sort(values(tags_dict))
  endfunction

  function! s:get_tags_files() abort
    let tags_files = []
    for tags_path in s:get_doc_tags()
      call extend(tags_files, filter(map(split(&runtimepath, ','), 'v:val . tags_path'), 'filereadable(v:val)'))
    endfor
    return tags_files
  endfunction

  function! s:help_tags_sink(line) abort
    let tag = get(split(a:line, "\t"), 0)
    execute 'help' tag
  endfunction
endif

let s:help_tags = {}
let s:help_tags.sink = function('s:help_tags_sink')
let s:help_tags.source = function('s:help_tags_source')

let g:clap#provider#help_tags# = s:help_tags

let &cpoptions = s:save_cpo
unlet s:save_cpo
