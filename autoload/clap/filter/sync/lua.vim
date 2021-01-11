" Author: liuchengxu <xuliuchengxlc@gmail.com>
" Description: Lua implementation of fzy filter algorithm.

let s:save_cpo = &cpoptions
set cpoptions&vim

if has('nvim-0.5')

  function! clap#filter#sync#lua#(query, candidates, _winwidth, enable_icon, match_type) abort
    let g:_clap_lua_query = a:query
    let g:_clap_lua_candidates = a:candidates
    let g:_clap_lua_enable_icon = a:enable_icon
    let g:_clap_lua_match_type = a:match_type

lua << EOF
local fzy_filter = require('fzy_filter')
vim.g.__clap_fuzzy_matched_indices, vim.g._clap_lua_filtered =
    fzy_filter.do_fuzzy_match(vim.g._clap_lua_query, vim.g._clap_lua_candidates, vim.g._clap_lua_enable_icon, vim.g._clap_lua_match_type)
EOF

    return g:_clap_lua_filtered
  endfunction

else

  function! s:deconstrcut(joint_indices) abort
    return map(split(a:joint_indices, ','), 'str2nr(v:val)')
  endfunction

  function! clap#filter#sync#lua#(query, candidates, _winwidth, enable_icon, match_type) abort
lua << EOF
local fzy_filter = require('fzy_filter')

local candidates = vim.eval('a:candidates')

local lines = {}
for i = #candidates-1, 0, -1 do
  table.insert(lines, candidates[i])
end

matched_indices, _clap_lua_filtered =
    fzy_filter.do_fuzzy_match(vim.eval('a:query'), lines, vim.eval('a:enable_icon'), vim.eval('a:match_type'))

__clap_fuzzy_matched_indices = {}
for i, v1 in ipairs(matched_indices) do
  local joint_indices = ''
  for _, v2 in ipairs(v1) do
    joint_indices = joint_indices .. v2 .. ','
  end
  table.insert(__clap_fuzzy_matched_indices, joint_indices)
end
EOF

    " TODO: vim.list() can not work with a List of List.
    " echom string(luaeval('vim.list(__clap_fuzzy_matched_indices)'))

    let g:__clap_fuzzy_matched_indices = map(luaeval('vim.list(__clap_fuzzy_matched_indices)'), 's:deconstrcut(v:val)')

    return luaeval('vim.list(_clap_lua_filtered)')
  endfunction

endif

let &cpoptions = s:save_cpo
unlet s:save_cpo
