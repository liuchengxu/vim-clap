" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Lua implementation of fzy filter algorithm.

let s:save_cpo = &cpoptions
set cpoptions&vim

" TODO: Older neovim & Vim support?
if has('nvim')

  function! clap#filter#sync#lua#(query, candidates, _winwidth, enable_icon, _line_splitter) abort
    let g:_clap_lua_query = a:query
    let g:_clap_lua_candidates = a:candidates
    let g:_clap_lua_enable_icon = a:enable_icon

lua << EOF
local fzy_filter = require('fzy_filter')
vim.g.__clap_fuzzy_matched_indices, vim.g._clap_lua_filtered =
    fzy_filter.do_fuzzy_match(vim.g._clap_lua_query, vim.g._clap_lua_candidates, vim.g._clap_lua_enable_icon)
EOF

    return g:_clap_lua_filtered
  endfunction

else

  function! clap#filter#sync#lua#(query, candidates, _winwidth, enable_icon, _line_splitter) abort
lua << EOF
local fzy_filter = require('fzy_filter')

local candidates = vim.eval('a:candidates')

local lines = {}
for i = #candidates-1, 0, -1 do
  table.insert(lines, candidates[i])
end

_clap_fuzzy_matched_indices, _clap_lua_filtered =
    fzy_filter.do_fuzzy_match(vim.eval('a:query'), lines, vim.eval('a:enable_icon'))

__clap_fuzzy_matched_indices = {}
for i, v1 in ipairs(_clap_fuzzy_matched_indices) do
  local joint_indices = ''
  for _, v2 in ipairs(v1) do
    joint_indices = joint_indices .. v2 .. ','
  end
  table.insert(__clap_fuzzy_matched_indices, joint_indices)
end
EOF

    " TODO: vim.list() can not work with a List of List.
    " echom string(luaeval('vim.list(__clap_fuzzy_matched_indices)'))

    let g:__clap_fuzzy_matched_indices = []
    for joint_indices in luaeval('vim.list(__clap_fuzzy_matched_indices)')
      call add(g:__clap_fuzzy_matched_indices, map(split(joint_indices, ','), 'str2nr(v:val)'))
    endfor

    return luaeval('vim.list(_clap_lua_filtered)')
  endfunction

endif

let &cpoptions = s:save_cpo
unlet s:save_cpo
