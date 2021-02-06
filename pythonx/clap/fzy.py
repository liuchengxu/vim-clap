#!/usr/bin/env python
# -*- coding: utf-8 -*-

import vim
from clap.scorer import fzy_scorer, substr_scorer


def str2bool(v):
    #  For neovim, vim.eval("a:enable_icon") is str
    #  For vim, vim.eval("a:enable_icon") is bool
    if isinstance(v, bool):
        return v
    else:
        return v.lower() in ("yes", "true", "t", "1")


def apply_score(scorer, query, candidates, enable_icon):
    scored = []

    for c in candidates:
        #  Skip two chars
        if enable_icon:
            candidate = c[2:]
        else:
            candidate = c
        score, indices = scorer(query, candidate)
        if score != float("-inf"):
            if enable_icon:
                indices = [x + 4 for x in indices]
            scored.append({'score': score, 'indices': indices, 'text': c})

    return scored


def fuzzy_match_py(query, candidates, enable_icon):
    if ' ' in query:
        scorer = substr_scorer
    else:
        scorer = fzy_scorer

    scored = apply_score(scorer, query, candidates, enable_icon)
    ranked = sorted(scored, key=lambda x: x['score'], reverse=True)

    indices = []
    filtered = []
    for r in ranked:
        filtered.append(r['text'])
        indices.append(r['indices'])

    return (indices, filtered)


def clap_fzy_py():
    return fuzzy_match_py(vim.eval("a:query"), vim.eval("a:candidates"),
                          str2bool(vim.eval("a:context")['enable_icon']))


try:
    from clap.fuzzymatch_rs import fuzzy_match as fuzzy_match_rs

    def clap_fzy_rs():
        return fuzzy_match_rs(vim.eval("a:query"), vim.eval("a:candidates"),
                              vim.eval("a:recent_files"),
                              vim.eval("a:context"))
except Exception:
    pass
