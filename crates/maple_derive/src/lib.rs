mod impls;

use proc_macro::TokenStream;

#[proc_macro_derive(ClapPlugin, attributes(clap_plugin))]
pub fn clap_plugin_derive(input: TokenStream) -> TokenStream {
    match syn::parse(input) {
        Ok(ast) => impls::clap_plugin::clap_plugin_derive_impl(&ast),
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro_attribute]
pub fn subscriptions(_attr: TokenStream, item: TokenStream) -> TokenStream {
    impls::subscriptions::subscriptions_impl(item)
}
