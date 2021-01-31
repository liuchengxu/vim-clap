local fzy = require("fzy_impl")

local fzy_filter = {}

local function dump(o)
    if type(o) == "table" then
        local s = "{ "
        for k, v in pairs(o) do
            if type(k) ~= "number" then
                k = '"' .. k .. '"'
            end
            s = s .. "[" .. k .. "] = " .. dump(v) .. ","
        end
        return s .. "} "
    else
        return tostring(o)
    end
end

--
-- Find the last instance of a pattern in a string.
-- https://github.com/premake/premake-4.x/blob/7a6c9b6e1e357250671886c781ba351a4b804207/src/base/string.lua#L31
function string.findlast(s, pattern, plain)
  local curr = 0
  repeat
    local next = s:find(pattern, curr + 1, plain)
    if (next) then curr = next end
  until (not next)
  if (curr > 0) then
    return curr
  end
end

--
-- Retrieve the filename portion of a path.
--
-- https://github.com/premake/premake-4.x/blob/master/src/base/path.lua#L72
local function get_filename(p)
  local i = p:findlast("[/\\]")
  if (i) then
    return { filename = p:sub(i + 1), offset = i}
  else
    return { filename = p, offset = 0 }
  end
end

local function match_text_for(item, match_type)
  if match_type == 'FileNameOnly' then
    filename_info = get_filename(item)
    return { match_text = filename_info['filename'], offset = filename_info['offset'] }
  else
    return { match_text = item, offset = 0 }
  end
end

local function apply_fzy(query, candidates, enable_icon, match_type)
    if string.match(query, '%u') then
        case_sensitive = true
    else
        case_sensitive = false
    end
    matches = {}

    for _, c in pairs(candidates) do
        raw_c = c

        -- Skip two chars, icon + one Space
        if enable_icon then
            c = string.sub(c, 5)
            match_info = match_text_for(c, match_type)
            c = match_info['match_text']
            offset = match_info['offset'] + 3
        else
            match_info = match_text_for(c, match_type)
            c = match_info['match_text']
            offset = match_info['offset'] - 1
        end

        -- Enable case_sensitive
        if fzy.has_match(query, c, case_sensitive) then
            positions, score = fzy.positions(query, c)
            if score ~= fzy.get_score_min() then
                adjusted_positions = {}
                for i, v in ipairs(positions) do
                    adjusted_positions[i] = v + offset
                end
                table.insert(matches, {text = raw_c, score = score, indices = adjusted_positions})
            end
        end
    end

    return matches
end

local function compare(a, b)
    return a[2]["score"] > b[2]["score"]
end

function fzy_filter.do_fuzzy_match(query, candidates, enable_icon, match_type)
    -- https://cesarbs.org/blog/2009/10/23/why-luas-0-zero-as-a-true-value-makes-sense/
    if enable_icon == 0 then
      enable_icon = false
    end

    scored = apply_fzy(query, candidates, enable_icon, match_type)

    ranked = {}
    for k, v in pairs(scored) do
        table.insert(ranked, {k, v})
    end
    table.sort(ranked, compare)

    indices = {}
    filtered = {}
    for k, v in pairs(ranked) do
        table.insert(indices, v[2].indices)
        table.insert(filtered, v[2].text)
    end

    return indices, filtered
end

return fzy_filter
