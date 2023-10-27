use std::collections::HashSet;

use darling::FromMeta;
use proc_macro::{self, TokenStream};
use proc_macro2::Span;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{DeriveInput, Error, Ident, LitStr, Meta, Token};

#[proc_macro_derive(ClapPlugin, attributes(action, actions, clap_plugin))]
pub fn clap_plugin_derive(input: TokenStream) -> TokenStream {
    match syn::parse(input) {
        Ok(ast) => clap_plugin_derive_impl(&ast),
        Err(e) => e.to_compile_error().into(),
    }
}

#[derive(Debug, Eq, PartialEq, FromMeta)]
struct Plugin {
    id: LitStr,
}

fn clap_plugin_derive_impl(input: &DeriveInput) -> TokenStream {
    let mut action_parsed = Vec::<String>::new();
    let mut actions_parsed = Vec::<String>::new();

    let mut maybe_plugin_id = None;

    // Extract the attribute values from the struct level
    for attr in &input.attrs {
        if attr.path().is_ident("clap_plugin") {
            let plugin = Plugin::from_meta(&attr.meta).expect("Invalid clap_plugin attribute");
            maybe_plugin_id.replace(plugin.id.value());
        }

        if attr.path().is_ident("action") {
            let lit: LitStr = attr.parse_args().expect("Failed to parse action args");
            action_parsed.push(lit.value());
        }

        if attr.path().is_ident("actions") {
            if let Meta::List(list) = &attr.meta {
                let args = list
                    .parse_args_with(Punctuated::<LitStr, Token![,]>::parse_terminated)
                    .expect("Failed to parse actions args");
                let args = args.iter().map(|arg| arg.value()).collect::<Vec<_>>();
                let _ = std::mem::replace(&mut actions_parsed, args);
            }
        }
    }

    let plugin_id = maybe_plugin_id.expect("Plugin id must be specified");

    // Combine #[action(..)] and #[actions(..)]
    let mut args_parsed = action_parsed;
    args_parsed.extend(actions_parsed);

    let DeriveInput { ident, .. } = input;

    // No actions specified.
    if args_parsed.is_empty() {
        let output = quote! {
            impl types::ClapAction for #ident {
                fn id(&self) -> &'static str {
                    #plugin_id
                }

                fn actions(&self, _action_type: types::ActionType) -> &[types::Action] {
                  &[]
                }
            }
        };

        return output.into();
    }

    let mut actions_list = Vec::new();
    let mut callable_actions_list = Vec::new();
    let mut internal_actions_list = Vec::new();

    let mut used_actions = HashSet::new();

    // Generate constants from the attribute values
    let constants = args_parsed.iter().map(|action| {
        let action_name = action.as_str();

        if used_actions.contains(action_name) {
            return Error::new(
                Span::call_site(),
                format!("Duplicated action ({action_name}) in plugin {plugin_id}"),
            )
            .to_compile_error();
        } else {
            used_actions.insert(action_name);
        }

        // Classify the action and extract the operation.
        let (is_callable, action_operation) =
            if let Some(action_operation) = action_name.strip_prefix("__") {
                (false, action_operation)
            } else {
                (true, action_name)
            };

        let check_operation_validity = |operation: &str| {
            let is_valid = operation
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');

            if is_valid {
                None
            } else {
                Some(Error::new(
                    Span::call_site(),
                    format!("Invalid character in {action_name}: expect only ASCII alphanumeric character or [-_]"),
                ))
            }
        };


        if let Some(err) = check_operation_validity(action_operation) {
            return err.to_compile_error();
        }

        let action_name = action_name.replace('-', "_");

        let uppercase_action = action_name.to_uppercase();
        let action_lit = Ident::new(&uppercase_action, ident.span());
        let action_var = Ident::new(&format!("ACTION_{uppercase_action}"), ident.span());

        actions_list.push(action_var.clone());

        // No prefix for system plugin.
        let namespaced_action = if plugin_id == "system" {
            action.clone()
        } else {
            format!("{plugin_id}/{action}")
        };

        if is_callable {
            callable_actions_list.push(action_var.clone());

            quote! {
                const #action_lit: &str = #namespaced_action;
                const #action_var: types::Action = types::Action::callable(Self::#action_lit);
            }
        } else {
            internal_actions_list.push(action_var.clone());

            quote! {
                const #action_lit: &str = #namespaced_action;
                #[allow(non_upper_case_globals)]
                const #action_var: types::Action = types::Action::internal(Self::#action_lit);
            }
        }
    });

    let output = quote! {
        impl #ident {
            #(#constants)*

            const CALLABLE_ACTIONS: &[types::Action] = &[#(Self::#callable_actions_list),*];
            const INTERNAL_ACTIONS: &[types::Action] = &[#(Self::#internal_actions_list),*];
            const ACTIONS: &[types::Action] = &[#(Self::#actions_list),*];

        }

        impl types::ClapAction for #ident {
            fn id(&self) -> &'static str {
                #plugin_id
            }

            fn actions(&self, action_type: types::ActionType) -> &[types::Action] {
                use types::ActionType;

                match action_type {
                    ActionType::Callable => Self::CALLABLE_ACTIONS,
                    ActionType::Internal => Self::INTERNAL_ACTIONS,
                    ActionType::All => Self::ACTIONS,
                }
            }
        }

    };

    output.into()
}
