use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input, DeriveInput, LitStr};

#[proc_macro_derive(ClapPlugin, attributes(action))]
pub fn derive(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let input = parse_macro_input!(input as DeriveInput);

    let mut actions = Vec::<String>::new();

    // Extract the attribute values from the struct level
    for attr in input.attrs {
        if attr.path().is_ident("action") {
            let lit: LitStr = attr.parse_args().expect("Failed to parse MetaList");
            actions.push(lit.value());
        }
    }

    let DeriveInput { ident, .. } = input;

    // Generate constants from the attribute values
    let constants = actions.iter().map(|action| {
        let action_id = action.replace('/', "_");
        let constant_name = format!("ACTION_{}", action_id.to_uppercase());
        let constant_var = syn::Ident::new(&constant_name, ident.span());
        quote! {
            const #constant_var: &str = #action;
        }
    });

    let output = quote! {
        impl #ident {
            #(#constants)*
        }
    };

    output.into()
}
