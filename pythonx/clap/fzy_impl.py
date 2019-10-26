#!/usr/bin/env python
# -*- coding: utf-8 -*-

from functools import partial

#  Credit: https://github.com/aslpavel/sweep.py/blob/master/sweep.py
#
#  Fuzzy matching for `fzy` utility
#  source: https://github.com/jhawthorn/fzy/blob/master/src/match.c

SCORE_MIN = float("-inf")
SCORE_MAX = float("inf")
SCORE_GAP_LEADING = -0.005
SCORE_GAP_TRAILING = -0.005
SCORE_GAP_INNER = -0.01
SCORE_MATCH_CONSECUTIVE = 1.0


def char_range_with(c_start, c_stop, v, d):
    d = d.copy()
    d.update((chr(c), v) for c in range(ord(c_start), ord(c_stop) + 1))
    return d


lower_with = partial(char_range_with, "a", "z")
upper_with = partial(char_range_with, "A", "Z")
digit_with = partial(char_range_with, "0", "9")

SCORE_MATCH_SLASH = 0.9
SCORE_MATCH_WORD = 0.8
SCORE_MATCH_CAPITAL = 0.7
SCORE_MATCH_DOT = 0.6
BONUS_MAP = {
    "/": SCORE_MATCH_SLASH,
    "-": SCORE_MATCH_WORD,
    "_": SCORE_MATCH_WORD,
    " ": SCORE_MATCH_WORD,
    ".": SCORE_MATCH_DOT,
}
BONUS_STATES = [{}, BONUS_MAP, lower_with(SCORE_MATCH_CAPITAL, BONUS_MAP)]
BONUS_INDEX = digit_with(1, lower_with(1, upper_with(2, {})))


def bonus(haystack):
    """
    Additional bonus based on previous char in haystack
    """
    c_prev = "/"
    bonus = []
    for c in haystack:
        bonus.append(BONUS_STATES[BONUS_INDEX.get(c, 0)].get(c_prev, 0))
        c_prev = c
    return bonus


def subsequence(niddle, haystack):
    """
    Check if niddle is subsequence of haystack
    """
    niddle, haystack = niddle.lower(), haystack.lower()
    if not niddle:
        True
    offset = 0
    for char in niddle:
        offset = haystack.find(char, offset) + 1
        if offset <= 0:
            return False
    return True


def compute(niddle, haystack):
    """
    Calculate score, and positions of haystack
    """
    n, m = len(niddle), len(haystack)
    bonus_score = bonus(haystack)
    niddle, haystack = niddle.lower(), haystack.lower()

    if n == 0 or n == m:
        return SCORE_MAX, list(range(n))

    D = [[0] * m for _ in range(n)]  # best score ending with `niddle[:i]`
    M = [[0] * m for _ in range(n)]  # best score for `niddle[:i]`

    for i in range(n):
        prev_score = SCORE_MIN
        gap_score = SCORE_GAP_TRAILING if i == n - 1 else SCORE_GAP_INNER

        for j in range(m):
            if niddle[i] == haystack[j]:
                score = SCORE_MIN
                if i == 0:
                    score = j * SCORE_GAP_LEADING + bonus_score[j]
                elif j != 0:
                    score = max(
                        M[i - 1][j - 1] + bonus_score[j],
                        D[i - 1][j - 1] + SCORE_MATCH_CONSECUTIVE,
                    )
                D[i][j] = score
                M[i][j] = prev_score = max(score, prev_score + gap_score)
            else:
                D[i][j] = SCORE_MIN
                M[i][j] = prev_score = prev_score + gap_score

    return D, M


def positions(niddle, haystack):
    n, m = len(niddle), len(haystack)

    positions = [0] * n

    if n == 0 or m == 0:
        return positions

    if n == m:
        return positions

    if m > 1024:
        return positions

    match_required = False

    D, M = compute(niddle, haystack)

    i, j = n - 1, m - 1

    while i >= 0:
        while j >= 0:
            if (match_required or D[i][j] == M[i][j]) and D[i][j] != SCORE_MIN:
                match_required = (i > 0 and j > 0
                                  and M[i][j] == D[i - 1][j - 1] +
                                  SCORE_MATCH_CONSECUTIVE)
                positions[i] = j
                j -= 1
                break
            else:
                j -= 1
        i -= 1

    return M[n - 1][m - 1], positions


def score(niddle, haystack):
    """
    Calculate score, and positions of haystack
    """
    n, m = len(niddle), len(haystack)
    bonus_score = bonus(haystack)
    niddle, haystack = niddle.lower(), haystack.lower()

    if n == 0 or n == m:
        return SCORE_MAX, list(range(n))
    D = [[0] * m for _ in range(n)]  # best score ending with `niddle[:i]`
    M = [[0] * m for _ in range(n)]  # best score for `niddle[:i]`
    for i in range(n):
        prev_score = SCORE_MIN
        gap_score = SCORE_GAP_TRAILING if i == n - 1 else SCORE_GAP_INNER

        for j in range(m):
            if niddle[i] == haystack[j]:
                score = SCORE_MIN
                if i == 0:
                    score = j * SCORE_GAP_LEADING + bonus_score[j]
                elif j != 0:
                    score = max(
                        M[i - 1][j - 1] + bonus_score[j],
                        D[i - 1][j - 1] + SCORE_MATCH_CONSECUTIVE,
                    )
                D[i][j] = score
                M[i][j] = prev_score = max(score, prev_score + gap_score)
            else:
                D[i][j] = SCORE_MIN
                M[i][j] = prev_score = prev_score + gap_score

    match_required = False
    positions = [0] * n
    i, j = n - 1, m - 1
    while i >= 0:
        while j >= 0:
            if (match_required or D[i][j] == M[i][j]) and D[i][j] != SCORE_MIN:
                match_required = (i > 0 and j > 0
                                  and M[i][j] == D[i - 1][j - 1] +
                                  SCORE_MATCH_CONSECUTIVE)
                positions[i] = j
                j -= 1
                break
            else:
                j -= 1
        i -= 1

    return M[n - 1][m - 1], positions


def fzy_scorer(niddle, haystack):
    if subsequence(niddle, haystack):
        return score(niddle, haystack)
    else:
        return SCORE_MIN, None
