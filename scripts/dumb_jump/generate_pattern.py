#!/usr/bin/env python3
# -*- coding: utf-8 -*-

import re
import os.path
import json

if os.path.isfile("dumb-jump.el"):
    lines = open('dumb-jump.el').readlines()
    lines = [line[:-1] for line in lines]
else:
    import urllib.request
    url = ('https://raw.githubusercontent.com/'
           'jacktasia/dumb-jump/master/dumb-jump.el')
    response = urllib.request.urlopen(url)
    lines = response.read().decode('utf-8').split("\n")

start_line = '(defcustom dumb-jump-find-rules'
stop_line = '(defcustom dumb-jump-language-contexts'
type_pattern = r':type\s+(.*):supports\s+(.*):language\s+(.*)'
regex_pattern = r':regex\s+"(.*)"'

start_idx = lines.index(start_line)

rules = {}

for idx in range(start_idx, len(lines)):
    if lines[idx + 1].strip().startswith(';;'):
        continue
    t = re.search(type_pattern, lines[idx])
    if t:
        regex = lines[idx + 1].split()[1][1:-1]

        regex = regex.replace('\\\\', "\\")

        ty = t.group(1).replace('"', '').strip()
        supports = t.group(2)
        language = t.group(3).replace('"', '').split()[0].strip()

        if language in rules:
            language_rule = rules[language]
            if ty in language_rule:
                language_rule[ty].append(regex)
            else:
                language_rule[ty] = [regex]
        else:
            rules[language] = {ty: [regex]}

    if lines[idx] == stop_line:
        break

special_lang_map = {'c++': 'cpp'}

rules['cpp'] = rules['c++']
rules.pop('c++', None)

with open('rg_pcre2_regex.json', 'w') as f:
    json.dump(rules, f, indent=4)

print(rules.keys())

comments_map = {
    '*': ['//'],
    'lua': ['--'],
    'erl': ['%'],
    'hrl': ['%'],
    'tex': ['%'],
    'r': ['//'],
    'go': ['//'],
    'rs': ['//', '//!', '///'],
    'cpp': ['//'],
    'javascript': ['//'],
    'typescript': ['//'],
    'php': ['//', '#'],
    'el': [';'],
    'clj': [';'],
    'exs': ['#'],
    'perl': ['#'],
    'py': ['#'],
    'nim': ['#'],
    'rb': ['#'],
}

with open('comments_map.json', 'w') as f:
    json.dump(comments_map, f, indent=4)
