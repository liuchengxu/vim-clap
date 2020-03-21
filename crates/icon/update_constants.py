#!/usr/bin/env python3
# -*- coding: utf-8 -*-

#  Whenever you change exactmatch_map.json or extension_map.json,
#  rerun this script to regenerate src/constants.rs:
#    ./update_constants.py
import json


#  Read exactmatch_map.json and extension_map.json into the form of
#  ("ai", ""),("awk", ""),("bash", ""), ordered by the key.
def into_joined_tuples(icon_json_source):
    with open(icon_json_source, 'r') as f:
        disordered = json.load(f)
        sorted_dict = {k: disordered[k] for k in sorted(disordered)}

        joined_tuples = ','.join(
            map(lambda kv: '("%s", "%s")' % (kv[0], kv[1]),
                sorted_dict.items()))
        return joined_tuples


with open('src/constants.rs', 'w') as f:
    decl_exactmatch_map = ("pub static ref EXACTMATCH_MAP: "
                           "HashMap<&'static str, &'static str>")
    exactmatches = into_joined_tuples('exactmatch_map.json')
    exactmatch_map_value = "[%s].iter().copied().collect()" % exactmatches

    decl_extension_map = ("pub static ref EXTENSION_MAP: "
                          "HashMap<&'static str, &'static str>")
    extensions = into_joined_tuples('extension_map.json')
    extension_map_value = "[%s].iter().copied().collect()" % extensions

    f.write("lazy_static! { %s = %s; %s = %s; }" %
            (decl_exactmatch_map, exactmatch_map_value, decl_extension_map,
             extension_map_value))
