#!/usr/bin/env python3
# -*- coding: utf-8 -*-

import itertools
import random
import re
import string

import fuzzymatch_rs
from fzy_impl import fzy_scorer


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


query = 'sr'
candidates = open('../../test/testdata.txt', 'r').read().split('\n')

print(fuzzy_match_py(query, candidates))
print(fuzzymatch_rs.fuzzy_match(query, candidates))


def test_pure_python_10000(benchmark):
    print(benchmark(fuzzy_match_py, query, candidates[:10000]))


def test_rust_10000(benchmark):
    print(benchmark(fuzzymatch_rs.fuzzy_match, query, candidates[:10000]))


def test_pure_python_100000(benchmark):
    print(benchmark(fuzzy_match_py, query, candidates[:100000]))


def test_rust_100000(benchmark):
    print(benchmark(fuzzymatch_rs.fuzzy_match, query, candidates[:100000]))


def test_pure_python_200000(benchmark):
    print(benchmark(fuzzy_match_py, query, candidates[:200000]))


def test_rust_200000(benchmark):
    print(benchmark(fuzzymatch_rs.fuzzy_match, query, candidates[:200000]))


#  This would cost more than 30 seconds for Python.
#  def test_pure_python_500000(benchmark):
#  print(benchmark(fuzzy_match_py, query, candidates[:500000]))

#  def test_rust_500000(benchmark):
#  print(benchmark(fuzzymatch_rs.fuzzy_match, query, candidates[:500000]))

#  def test_pure_python_800000(benchmark):
#  print(benchmark(fuzzy_match_py, query, candidates[:800000]))

#  def test_rust_800000(benchmark):
#  print(benchmark(fuzzymatch_rs.fuzzy_match, query, candidates[:800000]))
