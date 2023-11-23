use itertools::Itertools;
use maple_core::config::{
    Config, IgnoreConfig, LogConfig, MatcherConfig, PluginConfig, ProviderConfig,
};
use proc_macro2::Ident;
use quote::{quote, ToTokens};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use syn::{
    parse_quote, Attribute, Field, Fields, ItemStruct, Lit, Meta, MetaNameValue, NestedMeta,
    PathSegment, Type,
};

#[derive(Default, Debug, Serialize, Deserialize)]
struct DefaultConfig(Config);

impl quote::ToTokens for DefaultConfig {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let struct_ident = proc_macro2::Ident::new(
            &format!("{}", quote!(#self)),
            proc_macro2::Span::call_site(),
        );
        let default_impl = quote! {
            impl Default for #struct_ident {
                fn default() -> Self {
                  Self({
                      /// Ignore configuration per project.
                      ///
                      /// The project path must be specified as absolute path or a path relative to the home directory.
                      project_ignore: HashMap::new(),

                      /// Log configuration.
                      log: LogConfig::default(),

                      /// Matcher configuration.
                      matcher: MatcherConfig::default(),

                      /// Plugin configuration.
                      plugin: PluginConfig::default(),

                      /// Provider configuration.
                      provider: ProviderConfig::default(),

                      /// Global ignore configuration.
                      global_ignore: IgnoreConfig::default(),
                  })
                }
            }
        };
        default_impl.to_tokens(tokens);
        // self.to_tokens(tokens);
    }
}

// Function to extract doc comments for a given struct
fn extract_doc_comments<T: Default + Serialize + quote::ToTokens>() -> BTreeMap<String, String> {
    let default_struct = T::default();
    let struct_name = format!("{}", quote!(#default_struct).to_string());
    let ast = syn::parse_str::<ItemStruct>(&struct_name).unwrap();

    let mut comments_map = BTreeMap::new();

    for field in ast.fields.iter() {
        if let Some(comment) = &field
            .attrs
            .first()
            .and_then(|attr| attr.parse_meta().ok())
            .and_then(|meta| {
                if let Meta::NameValue(MetaNameValue {
                    path,
                    lit: syn::Lit::Str(lit),
                    ..
                }) = meta
                {
                    if path.is_ident("doc") {
                        return Some(lit.value());
                    }
                }
                None
            })
        {
            let field_name = field.ident.as_ref().unwrap().to_string();
            comments_map.insert(field_name, comment.trim().to_string());
        }
    }

    comments_map
}

fn main() {
    let default_config = Config::default();
    let toml_string = toml::to_string_pretty(&default_config).unwrap();

    println!("{toml_string}");

    let source_code = include_str!("../../maple_core/src/config.rs");
    // Parse the source code into an AST
    let ast: syn::File = syn::parse_str(source_code).expect("Failed to parse Rust source code");

    process_ast(&ast);
}

fn get_field_type_as_string(field: &Field) -> String {
    let mut tokens = quote::quote! {};
    field.ty.to_tokens(&mut tokens);

    tokens.to_string()
}

fn is_struct_type(field: &Field) -> bool {
    if let Type::Path(type_path) = &field.ty {
        let path = &type_path.path;
        if let Some(PathSegment { ident, arguments }) = path.segments.last() {
            // Check if the last path segment is an identifier (struct name)
            // and there are no type arguments (e.g., generic parameters)
            return arguments.is_empty();
        }
    }
    false
}

#[derive(Debug, Clone)]
struct FieldInfo {
    is_struct: Option<String>,
    /// Extracted doc comments on this field.
    docs: Vec<String>,
}

fn parse_struct(s: &ItemStruct) -> BTreeMap<String, FieldInfo> {
    let mut struct_docs = BTreeMap::new();
    match &s.fields {
        Fields::Named(named_fields) => {
            for field in &named_fields.named {
                let field_docs = field
                    .attrs
                    .iter()
                    .filter_map(extract_doc_comment)
                    .collect::<Vec<_>>();

                let field_type = get_field_type_as_string(field);

                let ident = field.ident.clone().unwrap();

                struct_docs.insert(
                    ident.to_string(),
                    FieldInfo {
                        is_struct: is_struct_type(field).then_some(field_type),
                        docs: field_docs,
                    },
                );
            }
        }
        _ => {}
    }
    struct_docs
}

fn process_ast(ast: &syn::File) {
    let mut all_struct_docs = BTreeMap::new();

    // You can now traverse the AST and perform actions based on its structure.
    // For example, let's print the names of all structs in the AST.
    for item in &ast.items {
        if let syn::Item::Struct(ref s) = item {
            println!("Found struct: {}", s.ident);
            let struct_docs = parse_struct(s);
            println!("struct {} docs: {:#?}", s.ident, struct_docs);

            for (field, field_info) in &struct_docs {
                println!(
                    "{}",
                    field_info
                        .docs
                        .iter()
                        .map(|line| format!("#{line}"))
                        .join("\n")
                );
                println!("[{field}]\n");
            }

            all_struct_docs.insert(s.ident.to_string(), struct_docs);
        }
    }

    let root_config_docs = all_struct_docs.get("Config").expect("Config not found");
    println!("{all_struct_docs:#?}");
    println!("{root_config_docs:#?}");

    for (field, field_info) in root_config_docs {
        let docs = field_info
            .docs
            .iter()
            .map(|line| format!("#{line}"))
            .join("\n");
        println!("{docs}\n[{field}]\n");

        if let Some(struct_name) = &field_info.is_struct {
            let struct_fields_docs = all_struct_docs.get(struct_name).expect("Struct not found");
            generate_nested_struct_config_docs(&field, struct_fields_docs, &all_struct_docs);
        }
    }
}

fn generate_nested_struct_config_docs(
    parent: &str,
    field_docs: &BTreeMap<String, FieldInfo>,
    all_struct_docs: &BTreeMap<String, BTreeMap<String, FieldInfo>>,
) {
    for (field, field_info) in field_docs {
        // println!("field: {field}, field_info: {field_info:?}");
        let docs = field_info
            .docs
            .iter()
            .map(|line| format!("#{line}"))
            .join("\n");
        println!("{docs}\n[{parent}.{field}]\n");
    }
}

fn extract_doc_comment(attr: &Attribute) -> Option<String> {
    if let Ok(meta) = attr.parse_meta() {
        if let Meta::NameValue(MetaNameValue {
            path,
            lit: Lit::Str(comment),
            ..
        }) = meta
        {
            if path.is_ident("doc") {
                return Some(comment.value());
            }
        }
    }
    None
}
