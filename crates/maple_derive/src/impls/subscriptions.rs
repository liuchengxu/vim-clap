#![allow(clippy::single_match)]

use proc_macro::{self, TokenStream};
use quote::quote;
use std::collections::HashSet;
use syn::{
    parse_macro_input, Expr, ExprCall, ExprMatch, Ident, Item, ItemFn, Local, Pat, Stmt, UsePath,
    UseTree,
};

fn extract_variants(expr_match: &ExprMatch) -> Vec<Ident> {
    let autocmd_variants = types::AutocmdEventType::variants();

    let mut handled_autocmds = Vec::new();

    // Extract enum variants from the match arms
    for arm in &expr_match.arms {
        match &arm.pat {
            Pat::Or(pat_or) => {
                for case in &pat_or.cases {
                    if let Pat::Ident(pat_ident) = case {
                        if autocmd_variants.contains(&pat_ident.ident.to_string().as_str()) {
                            handled_autocmds.push(pat_ident.ident.clone());
                        }
                    }
                }
            }
            Pat::Ident(pat_ident) => {
                if autocmd_variants.contains(&pat_ident.ident.to_string().as_str()) {
                    handled_autocmds.push(pat_ident.ident.clone());
                }
            }
            _ => {}
        }
    }

    assert!(
        !handled_autocmds.is_empty(),
        "Handled autocmds must not be empty if Match statement exists"
    );

    handled_autocmds
}

fn parse_use_path(use_path: &UsePath) -> Vec<Ident> {
    match use_path.tree.as_ref() {
        UseTree::Group(use_group) => use_group
            .items
            .iter()
            .filter_map(|i| {
                if let UseTree::Name(name) = i {
                    Some(name.ident.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<Ident>>(),
        _ => {
            panic!("must be `use AutocmdEventType::{{...}}`")
        }
    }
}

// let __ret: Result<()> = {..}
fn parse_local_stmt(local: &Local) -> Option<Vec<Ident>> {
    let mut maybe_imports = None::<Vec<Ident>>;

    if let Some(init) = &local.init {
        match init.expr.as_ref() {
            Expr::Block(expr_block) => {
                for stmt in &expr_block.block.stmts {
                    if let Stmt::Expr(expr, _) = stmt {
                        if let Expr::Match(expr_match) = expr {
                            let variants = extract_variants(expr_match);

                            if let Some(imports) = maybe_imports {
                                let imports: HashSet<_> = imports.into_iter().collect();
                                assert!(
                                    imports == HashSet::<Ident, _>::from_iter(variants.clone()),
                                    r#"variants in `use AutocmdEventType::{{...}}` must match the handled ones"#
                                );
                            }

                            return Some(variants);
                        }
                    } else {
                        match stmt {
                            Stmt::Item(Item::Use(item_use)) => match &item_use.tree {
                                UseTree::Path(use_path) => {
                                    if use_path.ident == "AutocmdEventType" {
                                        maybe_imports.replace(parse_use_path(use_path));
                                    }
                                }
                                _ => {}
                            },
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }

    None
}

fn parse_async_fn_expr_call(expr_call: &ExprCall) -> Option<Vec<Ident>> {
    let mut maybe_variants = None;

    // Box::pin
    match expr_call.func.as_ref() {
        Expr::Path(expr_path) => {
            let paths = expr_path
                .path
                .segments
                .iter()
                .map(|s| s.ident.clone())
                .collect::<Vec<_>>();
            assert_eq!(
                paths,
                vec!["Box", "pin"],
                "statement of async fn must be Box::pin(...)"
            );
        }
        _ => {
            unreachable!("statement must be Box::pin(...) which is Expr::Path(..)")
        }
    }

    for arg in &expr_call.args {
        match arg {
            // async move {..}
            Expr::Async(expr_async) => {
                for stmt in &expr_async.block.stmts {
                    if let Stmt::Local(local) = stmt {
                        if let Some(variants) = parse_local_stmt(local) {
                            maybe_variants.replace(variants);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    maybe_variants
}

pub fn subscriptions_impl(item: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let input_fn = parse_macro_input!(item as ItemFn);

    // TODO: improve the robustness
    if input_fn.sig.ident != "handle_autocmd" {
        panic!(
            "#[maple_derive::subscriptions] only works for `async fn handle_autocmd(&self, ...)`"
        );
    }

    // ```
    // #[async_trait::async_trait]
    // impl ClapPlugin for MyPlugin {
    //     async fn handle_autocmd(&self, ...) -> Result<()> {
    //         ...
    //     }
    // }
    // ```
    //
    // The above example will be expanded to
    //
    // ```
    // fn handle_autocmd<...>(...) -> ::core::pin::Pin<..> where ... {
    //     Box::pin(async move {
    //         ...
    //     })
    // }
    // ```
    assert!(
        input_fn.block.stmts.len() == 1,
        "The block of expanded async fn has only one statement `Box::pin(async move {{ ... }})` \
        otherwise async_trait is changed",
    );

    let maybe_variants = match &input_fn.block.stmts[0] {
        Stmt::Expr(Expr::Call(expr_call), _) => parse_async_fn_expr_call(expr_call),
        _ => unreachable!("statement must be a Expr::Call `Box::pin(...)`"),
    };

    let gen = if let Some(variants) = maybe_variants {
        // Generate the subscriptions function
        quote! {
            #input_fn

            fn subscriptions(&self) -> &[types::AutocmdEventType] {
                use types::AutocmdEventType;
                &[#(AutocmdEventType::#variants),*]
            }
        }
    } else {
        quote! {
            #input_fn
        }
    };

    gen.into()
}
