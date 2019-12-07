#!/usr/bin/env python
# -*- coding: utf-8 -*-

import vim

from .fzy_impl import fzy_scorer


def fuzzy_match(query, candidates):
    scored = []

    for c in candidates:
        score, indices = fzy_scorer(query, c)
        if score != float("-inf"):
            scored.append({'score': score, 'indices': indices, 'text': c})

    ranked = sorted(scored, key=lambda x: x['score'], reverse=True)

    indices = []
    filtered = []
    for r in ranked:
        filtered.append(r['text'])
        indices.append(r['indices'])

    return (indices, filtered)


def __filter_post_process(lines):
    if not lines:
        lines = [vim.eval('g:clap_no_matches_msg')]
        vim.command('let g:__clap_has_no_matches = v:true')
        vim.command('call g:clap.display.set_lines_lazy(%s)' % lines)
        vim.command('call clap#impl#refresh_matches_count("0")')
    else:
        preload_capacity = int(
            vim.eval('get(g:clap.display, "preload_capacity", 2*&lines)'))
        matches_cnt = str(len(lines))
        lines = lines[:preload_capacity]
        vim.command('call g:clap.display.set_lines_lazy(%s)' % lines)
        vim.command('call clap#impl#refresh_matches_count(%s)' % matches_cnt)


def __after_fuzzy_matched(indices, filtered):
    # Note the fuzzy matched indices
    line_count = int(vim.eval('g:clap.display.line_count()'))
    vim.command(
        "let g:__clap_fuzzy_matched_indices = %s" % indices[:line_count])
    __filter_post_process(filtered)


def clap_fzy():
    (indices, filtered) = fuzzy_match(
        vim.eval("a:query"), vim.eval("a:candidates"))
    __after_fuzzy_matched(indices, filtered)
