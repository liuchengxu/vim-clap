#!/usr/bin/env python3
# -*- coding: utf-8 -*-

import datetime
import sys

if len(sys.argv) != 3:
    print('  Usage: ./prepare_release.py next_git_tag next_maple_version')
    print('Example: ./prepare_release.py [v]0.8 0.1.8')
    exit(1)

next_tag = sys.argv[1]
if next_tag.startswith('v'):
    next_tag = next_tag[1:]
next_maple_version = sys.argv[2]


def write_back(lines, f):
    with open(f, 'w') as writer:
        writer.writelines(lines)


def read_file(fname):
    f = open(fname)
    return f.readlines()


#  install.sh
fname = 'install.sh'
lines = read_file(fname)
lines[4] = "version=v{version}\n".format(version=next_tag)
write_back(lines, fname)

#  install.ps1
fname = 'install.ps1'
lines = read_file(fname)
lines[2] = "$version = 'v{version}'\n".format(version=next_tag)
write_back(lines, fname)

#  plugin/clap.vim
fname = 'plugin/clap.vim'
lines = read_file(fname)
lines[3] = '" Version:   {version}\n'.format(version=next_tag)
write_back(lines, fname)

#  CHANGELOG.md
fname = 'CHANGELOG.md'
lines = read_file(fname)
now = datetime.datetime.now()
today = now.strftime("%Y-%m-%d")
release_header = '## [{version}] {today}'.format(version=next_tag, today=today)
lines.insert(4, "\n")
lines.insert(5, release_header)
lines.insert(6, "\n")
write_back(lines, fname)

#  Cargo.toml
fname = 'Cargo.toml'
lines = read_file(fname)
lines[4] = 'version = "{version}"\n'.format(version=next_maple_version)
write_back(lines, fname)
