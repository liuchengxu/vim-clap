use std::collections::HashSet;

use proc_macro::{self, TokenStream};
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{parse_macro_input, DeriveInput, Ident, LitStr, Meta, Token};

#[proc_macro_derive(ClapPlugin, attributes(action, actions))]
pub fn derive(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let input = parse_macro_input!(input as DeriveInput);

    let mut action_parsed = Vec::<String>::new();
    let mut actions_parsed = None::<Vec<String>>;

    // Extract the attribute values from the struct level
    for attr in input.attrs {
        if attr.path().is_ident("action") {
            let lit: LitStr = attr.parse_args().expect("Failed to parse MetaList");
            action_parsed.push(lit.value());
        }

        if attr.path().is_ident("actions") {
            if let Meta::List(list) = attr.meta {
                let args = list
                    .parse_args_with(Punctuated::<LitStr, Token![,]>::parse_terminated)
                    .expect("Parse MetaList");
                let args = args.iter().map(|arg| arg.value()).collect::<Vec<_>>();
                actions_parsed.replace(args);
            }
        }
    }

    let DeriveInput { ident, .. } = input;

    let mut actions_ident = Vec::new();

    let mut used_actions = HashSet::new();

    // Combine action(..) and actions(..)
    let mut args_parsed = action_parsed;
    args_parsed.extend(actions_parsed.unwrap_or_default());

    if args_parsed.is_empty() {
        return TokenStream::new();
    }

    // Generate constants from the attribute values
    let constants = args_parsed.iter().map(|action| {
        let (plugin_namespace, action_name) = if action.contains('/') {
            let mut parts = action.split('/');
            (
                parts
                    .next()
                    .expect("Bad action {action}: plugin_namespace not found"),
                parts
                    .next()
                    .expect("Bad action {action}, action_name not found"),
            )
        } else {
            ("system", action.as_str())
        };
        if used_actions.contains(action_name) {
            panic!("duplicate {action_name} in {plugin_namespace}");
        } else {
            used_actions.insert(action_name);
        }

        let action_lit = Ident::new(&action_name.to_uppercase(), ident.span());
        let action_var = Ident::new(
            &format!("ACTION_{}", action_name.to_uppercase()),
            ident.span(),
        );
        actions_ident.push(action_var.clone());
        quote! {
            const #action_lit: &str = #action;
            const #action_var: types::Action = types::Action::callable(Self::#action_lit);
        }
    });

    let output = quote! {
        impl #ident {
            #(#constants)*

            const ACTIONS: &[types::Action] = &[#(Self::#actions_ident),*];
        }
    };

    output.into()
}
