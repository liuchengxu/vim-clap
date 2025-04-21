use std::collections::HashSet;
use std::sync::Mutex;

use darling::FromMeta;
use inflections::case::{is_camel_case, to_kebab_case, to_pascal_case};
use once_cell::sync::Lazy;
use proc_macro::{self, TokenStream};
use proc_macro2::Span;
use quote::quote;
use syn::{DeriveInput, Error, Expr, Ident, LitStr};
use types::PLUGIN_ACTION_SEPARATOR;

static PLUGINS: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

#[derive(Debug, Eq, PartialEq, FromMeta)]
struct Plugin {
    id: LitStr,
    actions: Option<Expr>,
}

pub fn clap_plugin_derive_impl(input: &DeriveInput) -> TokenStream {
    let mut maybe_plugin_id = None;
    let mut actions_parsed = Vec::<String>::new();

    // Extract the attribute values from the struct level
    for attr in &input.attrs {
        if attr.path().is_ident("clap_plugin") {
            let plugin = Plugin::from_meta(&attr.meta).expect("Invalid clap_plugin attribute");
            maybe_plugin_id.replace(plugin.id.value());

            if let Some(actions) = plugin.actions {
                if let syn::Expr::Array(expr_array) = actions {
                    let args = expr_array
                        .elems
                        .iter()
                        .filter_map(|expr| match expr {
                            syn::Expr::Lit(lit) => String::from_value(&lit.lit).ok(),
                            _ => panic!("actions expected array of string literals"),
                        })
                        .collect::<Vec<String>>();
                    actions_parsed.extend(args);
                } else {
                    panic!("unexpected expr type, actions must be an expr of array")
                }
            }
        }
    }

    let plugin_id = maybe_plugin_id.expect("Plugin id must be specified");

    let mut registered_plugins = PLUGINS.lock().unwrap();
    if !registered_plugins.insert(plugin_id.to_string()) {
        panic!("Conflicting plugin id: {plugin_id}");
    }
    drop(registered_plugins);

    let DeriveInput { ident, .. } = input;

    // No actions specified.
    if actions_parsed.is_empty() {
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

    let mut raw_actions = Vec::new();

    let mut actions_list = Vec::new();
    let mut callable_actions_list = Vec::new();
    let mut internal_actions_list = Vec::new();

    let mut used_actions = HashSet::new();

    // Parse actions
    let constants = actions_parsed.iter().map(|action| {
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
            if !operation.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return Some(Error::new(
                    Span::call_site(),
                    format!("Invalid character in {action_name}: expect only ASCII alphanumeric character or [-]"),
                ));
            }

            if is_camel_case(operation) {
                None
            } else {
                Some(Error::new(Span::call_site(), format!("{action_name} is not in camelCase")))
            }
        };


        if let Some(err) = check_operation_validity(action_operation) {
            return err.to_compile_error();
        }

        raw_actions.push(action_name);

        // __internalAction => __internal-action => __internal_action
        let action_name = to_kebab_case(action_name).replace('-', "_");

        // __INTERNAL_ACTION
        let uppercase_action = action_name.to_uppercase();
        let action_lit = Ident::new(&uppercase_action, ident.span());
        let action_var = Ident::new(&format!("ACTION_{uppercase_action}"), ident.span());

        actions_list.push(action_var.clone());

        // No plugin_id prefix for system plugin.
        let namespaced_action = if plugin_id == "system" {
            action.clone()
        } else {
            format!("{plugin_id}{PLUGIN_ACTION_SEPARATOR}{action}")
        };

        if is_callable {
            callable_actions_list.push(action_var.clone());

            quote! {
                const #action_lit: &'static str = #namespaced_action;
                const #action_var: types::Action = types::Action::callable(Self::#action_lit);
            }
        } else {
            internal_actions_list.push(action_var.clone());

            quote! {
                const #action_lit: &'static str = #namespaced_action;
                #[allow(non_upper_case_globals)]
                const #action_var: types::Action = types::Action::internal(Self::#action_lit);
            }
        }
    }).collect::<Vec<_>>();

    let plugin_action = Ident::new(
        &format!("{}Action", to_pascal_case(&plugin_id)),
        ident.span(),
    );
    let mut plugin_action_variants = Vec::new();
    let action_variants = raw_actions
        .iter()
        .map(|arg| {
            // "__noteRecentFiles", "word-highlighter.__defineHighlights"
            let method = if plugin_id == "system" {
                arg.to_string()
            } else {
                format!("{plugin_id}{PLUGIN_ACTION_SEPARATOR}{arg}")
            };
            let pascal_name = if let Some(name) = arg.strip_prefix("__") {
                format!("__{}", to_pascal_case(name))
            } else {
                to_pascal_case(arg)
            };

            let variant = Ident::new(&pascal_name, ident.span());
            plugin_action_variants.push(variant.clone());

            quote! {
                #method => Ok(#plugin_action::#variant),
            }
        })
        .collect::<Vec<_>>();

    let output = quote! {

        enum #plugin_action {
          #(#plugin_action_variants),*
        }

        impl #ident {
            fn parse_action(&self, method: impl AsRef<str>) -> std::io::Result<#plugin_action> {
                match method.as_ref() {
                  #(#action_variants)*
                  unknown => Err(std::io::Error::other(format!("[{}] unknown action: {unknown}", #plugin_id))),
                }
            }
        }

        impl #ident {
            #(#constants)*

            const CALLABLE_ACTIONS: &'static [types::Action] = &[#(Self::#callable_actions_list),*];
            const INTERNAL_ACTIONS: &'static [types::Action] = &[#(Self::#internal_actions_list),*];
            const ACTIONS: &'static [types::Action] = &[#(Self::#actions_list),*];

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
