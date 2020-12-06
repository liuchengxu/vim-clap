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

local function apply_fzy(query, candidates, enable_icon)
    matches = {}

    for _, c in pairs(candidates) do
        raw_c = c

        -- Skip two chars, icon + one Space
        if enable_icon then
            c = string.sub(c, 5)
            offset = 3
        else
            offset = -1
        end

        -- Enable case_sensitive
        if fzy.has_match(query, c, true) then
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

function fzy_filter.do_fuzzy_match(query, candidates, enable_icon)
    scored = apply_fzy(query, candidates, enable_icon)

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
