use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Ident, LitStr};

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

    let mut actions_ident = Vec::new();

    // Generate constants from the attribute values
    let constants = actions.iter().map(|action| {
        let mut parts = action.split('/');
        let _plugin_name = parts.next().expect("Bad action, plugin_name not found");
        // TODO: Validate actions
        let action_name = parts.next().expect("Bad action, action_name not found");
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
