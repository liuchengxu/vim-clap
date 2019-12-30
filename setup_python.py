import sys
from os.path import normpath, join
import vim
plugin_root_dir = vim.eval('g:clap#autoload_dir')
python_root_dir = normpath(join(plugin_root_dir, '..', 'pythonx'))
sys.path.insert(0, python_root_dir)
import clap
