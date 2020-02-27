#!/usr/bin/env python
# -*- coding: utf-8 -*-

import vim

from clap.scorer import fzy_scorer, substr_scorer


def apply_score(scorer, query, candidates):
    scored = []

    for c in candidates:
        score, indices = scorer(query, c)
        if score != float("-inf"):
            scored.append({'score': score, 'indices': indices, 'text': c})

    return scored


def fuzzy_match_py(query, candidates):
    if ' ' in query:
        scorer = substr_scorer
    else:
        scorer = fzy_scorer

    scored = apply_score(scorer, query, candidates)
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
    from clap.fuzzymatch_rs import fuzzy_match as fuzzy_match_rs

    def str2bool(v):
        #  For neovim, vim.eval("a:enable_icon") is str
        #  For vim, vim.eval("a:enable_icon") is bool
        if isinstance(v, bool):
            return v
        else:
            return v.lower() in ("yes", "true", "t", "1")

    def clap_fzy_rs():
        return fuzzy_match_rs(vim.eval("a:query"), vim.eval("a:candidates"),
                              int(vim.eval("a:winwidth")),
                              str2bool(vim.eval("a:enable_icon")))
except Exception:
    pass
