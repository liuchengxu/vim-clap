#!/usr/bin/env python
# -*- coding: utf-8 -*-

import vim

from clap.fzy_impl import fzy_scorer


def fuzzy_match_py(query, candidates):
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


def clap_fzy_py():
    return fuzzy_match_py(vim.eval("a:query"), vim.eval("a:candidates"))


try:
    from .fuzzymatch_rs import fuzzy_match as fuzzy_match_rs

    def clap_fzy_rs():
        return fuzzy_match_rs(vim.eval("a:query"), vim.eval("a:candidates"))
except Exception:
    pass
