use itertools::Itertools;
use maple_core::config::{
    Config, IgnoreConfig, LogConfig, MatcherConfig, PluginConfig, ProviderConfig,
};
use proc_macro2::Ident;
use quote::{quote, ToTokens};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use syn::{
    parse_quote, Attribute, Field, Fields, ItemStruct, Lit, Meta, MetaNameValue, NestedMeta, Type,
    PathSegment
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

fn parse_struct(s: &ItemStruct) -> BTreeMap<Ident, Vec<String>> {
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
                println!("get filed_type as string: {:?}", field_type);

                // Format the type using quote!
                let mut formatted_ty = quote::quote! {};
                field.ty.to_tokens(&mut formatted_ty);

                // if let Type::Path(type_path) = &field.ty {
                // println!("============ ty: {:?}", type_path.path.segments);
                // }

                if is_struct_type(field) {
                    println!("filed type: {:?}", formatted_ty);
                }
                let ident = field.ident.clone().unwrap();
                struct_docs.insert(ident, field_docs);
            }
        }
        _ => {}
    }
    struct_docs
}

fn process_ast(ast: &syn::File) {
    // You can now traverse the AST and perform actions based on its structure.
    // For example, let's print the names of all structs in the AST.
    for item in &ast.items {
        if let syn::Item::Struct(ref s) = item {
            println!("Found struct: {}", s.ident);
            let struct_docs = parse_struct(s);
            println!("struct {} docs: {:#?}", s.ident, struct_docs);

            for (field, field_docs) in struct_docs {
                println!(
                    "{}",
                    field_docs.iter().map(|line| format!("#{line}")).join("\n")
                );
                println!("[{field}]\n");
            }
        }
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
