use inflections::case::to_snake_case;
use itertools::Itertools;
use maple_config::Config;
use quote::ToTokens;
use std::collections::BTreeMap;
use std::str::FromStr;
use syn::{
    Attribute, Expr, Field, Fields, ItemEnum, ItemStruct, Lit, MetaNameValue, PathSegment, Type,
};
use toml_edit::DocumentMut;

fn main() {
    let source_code = include_str!("../../src/lib.rs");

    // Parse the source code into an AST
    let ast: syn::File = syn::parse_str(source_code).expect("Failed to parse Rust source code");

    let doc = process_ast(&ast);

    // Print the modified TOML document
    println!("{doc}");

    if cfg!(debug_assertions) {
        let default_config_toml = std::env::current_dir()
            .expect("Invalid current working directory")
            .join("default_config.toml");

        println!("Writing to: {}", default_config_toml.display());

        std::fs::write(default_config_toml, doc.to_string().trim().as_bytes())
            .expect("Unable to write default_config.toml");
    }

    let current_dir = std::env::current_dir().unwrap();
    let config_md = current_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("docs")
        .join("src")
        .join("plugins")
        .join("config.md");

    if !config_md.exists() {
        panic!(
            "../../../docs/src/plugins/config.md not found, cwd: {}",
            current_dir.display()
        );
    }

    let s = std::fs::read_to_string(&config_md).unwrap();
    // The convention in config.md is there is one and only one toml code block, which will be
    // periodly updated using the auto-generated toml above.
    let mut config_md_content = s
        .split('\n')
        .take_while(|line| !line.starts_with("```toml"))
        .collect::<Vec<_>>()
        .into_iter()
        .join("\n");

    config_md_content.push_str("\n```toml");
    config_md_content.push_str(doc.to_string().as_str());
    config_md_content.push_str("```");

    std::fs::write(config_md, config_md_content.as_bytes()).expect("Failed to write config.md");
}

fn is_struct_type(field: &Field) -> bool {
    if let Type::Path(type_path) = &field.ty {
        let path = &type_path.path;
        if let Some(PathSegment {
            ident: _,
            arguments,
        }) = path.segments.last()
        {
            // Check if the last path segment is an identifier (struct name)
            // and there are no type arguments (e.g., generic parameters)
            return arguments.is_empty();
        }
    }
    false
}

#[derive(Debug, Clone)]
struct FieldInfo {
    /// Represents whether this field is a struct.
    struct_type: Option<String>,
    /// Extracted doc comments on this field.
    docs: Vec<String>,
}

impl FieldInfo {
    fn as_toml_comments(&self) -> String {
        self.docs.iter().map(|line| format!("#{line}")).join("\n")
    }
}

fn extract_doc_comment(attr: &Attribute) -> Option<String> {
    if let Ok(MetaNameValue { path, value, .. }) = attr.meta.require_name_value() {
        if let Expr::Lit(expr_lit) = value {
            if let Lit::Str(comment) = &expr_lit.lit {
                if path.is_ident("doc") {
                    return Some(comment.value());
                }
            }
        }
    }
    None
}

/// Returns a map of (field, field_info) in the config struct.
fn parse_struct(s: &ItemStruct) -> BTreeMap<String, FieldInfo> {
    let mut struct_docs = BTreeMap::new();

    let get_field_type_as_string = |field: &Field| -> String {
        let mut tokens = quote::quote! {};
        field.ty.to_tokens(&mut tokens);
        tokens.to_string()
    };

    if let Fields::Named(named_fields) = &s.fields {
        for field in &named_fields.named {
            let field_docs = field
                .attrs
                .iter()
                .filter_map(extract_doc_comment)
                .collect::<Vec<_>>();

            let field_type = get_field_type_as_string(field);

            let ident = field
                .ident
                .as_ref()
                .expect("Config struct must use named field");

            struct_docs.insert(
                ident.to_string(),
                FieldInfo {
                    struct_type: is_struct_type(field).then_some(field_type),
                    docs: field_docs,
                },
            );
        }
    }

    struct_docs
}

/// Returns a map of (field, field_info) in an enum.
#[allow(unused)]
fn parse_enum(e: &ItemEnum) -> BTreeMap<String, FieldInfo> {
    e.variants
        .iter()
        .map(|variant| {
            let docs = variant
                .attrs
                .iter()
                .filter_map(extract_doc_comment)
                .collect();
            let variant_name = &variant.ident;
            (
                variant_name.to_string(),
                FieldInfo {
                    struct_type: None,
                    docs,
                },
            )
        })
        .collect()
}

/// Process `config.rs` to generate `default_config.toml`
///
/// Conventions:
/// - All structs with a suffix of `Config` are considered as part of the config file.
/// - `Config` struct is the entry of various configs.
fn process_ast(ast: &syn::File) -> DocumentMut {
    let mut all_struct_docs = BTreeMap::new();

    // Traverse the AST and perform actions on each struct.
    for item in &ast.items {
        if let syn::Item::Struct(ref s) = item {
            let ident_string = s.ident.to_string();

            if !ident_string.ends_with("Config") {
                println!("Ignoring non-Config struct");
            }

            let struct_docs = parse_struct(s);
            all_struct_docs.insert(ident_string, struct_docs);
        }
    }

    let root_config_docs = all_struct_docs.get("Config").expect("Config not found");

    // Inject the extracted docs into the default toml config.
    let default_config_toml =
        toml::to_string(&Config::default()).expect("Failed to convert Config::default() to toml");
    let mut doc = toml_edit::DocumentMut::from_str(&default_config_toml)
        .expect("Must be valid toml as it was just constructed internally");

    // Iterate the fields in Config: log, matcher, plugin, provider, global_ignore
    for (key, item) in doc.as_table_mut().iter_mut() {
        let field = to_snake_case(key.get());
        let field_info = root_config_docs
            .get(&field)
            .expect("Field missing in Config");

        let comments = field_info.as_toml_comments();

        let struct_type = field_info
            .struct_type
            .as_ref()
            .expect("Each field in Config is a struct until it's not'");

        let struct_docs = all_struct_docs
            .get(struct_type)
            .unwrap_or_else(|| panic!("{struct_type} not found in all_struct_docs"));

        if let Some(table) = item.as_table_mut() {
            // Add comments on top of [log], [matcher], [plugin], etc.
            table.decor_mut().set_prefix(format!("\n#{comments}\n"));

            for (mut t_key, t_item) in table.iter_mut() {
                // Fields like `max_level`, `log_target` in log { max_level, log_target }.
                let docs_key = to_snake_case(t_key.get());

                let struct_field = struct_docs.get(&docs_key).unwrap();

                // ctags: CtagsPluginConfig
                if let Some(inner_struct_type) = &struct_field.struct_type {
                    if let Some(struct_docs) = all_struct_docs.get(inner_struct_type) {
                        if let Some(t) = t_item.as_table_mut() {
                            for (mut t_key, item) in t.iter_mut() {
                                let comments = struct_docs
                                    .get(&to_snake_case(t_key.get()))
                                    .unwrap()
                                    .as_toml_comments();

                                // Ugly workaround to handle the special case `SyntaxPluginConfig
                                // { render_strategy }`.
                                if t_key.get() == "render-strategy"
                                    || t_key.get() == "language-server"
                                {
                                    if let Some(t) = item.as_table_mut() {
                                        t.decor_mut().set_prefix(format!("\n{comments}\n"));
                                    }
                                } else {
                                    t_key.leaf_decor_mut().set_prefix(format!("{comments}\n"));
                                }
                            }
                        }
                    }
                }

                if struct_field.docs.is_empty() {
                    continue;
                }

                let comments = struct_field.as_toml_comments();

                if let Some(t) = t_item.as_table_mut() {
                    t.decor_mut().set_prefix(format!("\n{comments}\n"));

                    for (mut t_key, t_item) in t.iter_mut() {
                        let docs_key = to_snake_case(t_key.get());

                        if let Some(s) = struct_docs.get(&docs_key) {
                            if s.docs.is_empty() {
                                continue;
                            }

                            let comments = s.as_toml_comments();

                            if let Some(t) = t_item.as_table_mut() {
                                t.decor_mut().set_prefix(format!("\n{comments}\n"));
                            } else if t_item.is_value() {
                                t_key.leaf_decor_mut().set_prefix(format!("{comments}\n"));
                            }
                        }
                    }
                } else if t_item.is_value() {
                    t_key.leaf_decor_mut().set_prefix(format!("{comments}\n"));
                }
            }
        }
    }

    doc
}
