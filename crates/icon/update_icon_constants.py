#!/usr/bin/env python3
# -*- coding: utf-8 -*-

import json


def into_joined_tuples(icon_json_source):
    with open(icon_json_source, 'r') as f:
        disordered = json.load(f)
        sorted_dict = {k: disordered[k] for k in sorted(disordered)}

        with open('sorted_extension_map.json', 'w') as fp:
            json.dump(sorted_dict, fp, indent=2)

        joined_tuples = ','.join(
            map(lambda kv: '("%s", "%s")' % (kv[0], kv[1]),
                sorted_dict.items()))
        return joined_tuples


joined_tuples = into_joined_tuples('exactmatch_map.json')

with open('src/exactmatch_map.rs', 'w') as f:
    decl_var = ("pub static ref EXACTMATCH_MAP: "
                "HashMap<&'static str, &'static str>")
    value = "[%s].iter().copied().collect()" % joined_tuples

    f.write("lazy_static! { %s = %s; }" % (decl_var, value))

joined_tuples = into_joined_tuples('extension_map.json')

with open('src/extension_map.rs', 'w') as f:
    decl_var = ("pub static ref EXTENSION_MAP: "
                "HashMap<&'static str, &'static str>")
    value = "[%s].iter().copied().collect()" % joined_tuples

    f.write("lazy_static! { %s = %s; }" % (decl_var, value))
