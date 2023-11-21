use maple_core::config::{
    Config, IgnoreConfig, LogConfig, MatcherConfig, PluginConfig, ProviderConfig,
};
use quote::quote;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use syn::{parse_quote, Attribute, ItemStruct, Lit, Meta, MetaNameValue, NestedMeta};

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

fn process_ast(ast: &syn::File) {
    // You can now traverse the AST and perform actions based on its structure.
    // For example, let's print the names of all structs in the AST.
    for item in &ast.items {
        if let syn::Item::Struct(ref s) = item {
            println!("Found struct: {}", s.ident);
            if s.ident == "Config" {
                for field in &s.fields {
                    let field_docs = field
                        .attrs
                        .iter()
                        .filter_map(extract_doc_comment)
                        .collect::<Vec<_>>();
                    let ident = field.ident.clone().unwrap();
                    println!("field ident: {ident}, docs: {}", field_docs.join("\n"));
                }
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
