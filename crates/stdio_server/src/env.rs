use super::types::{GlobalEnv, Message};
use log::debug;
use once_cell::sync::OnceCell;
use std::ops::Deref;

static GLOBAL_ENV: OnceCell<GlobalEnv> = OnceCell::new();

/// Ensure GLOBAL_ENV has been instalized before using it.
pub fn global() -> impl Deref<Target = GlobalEnv> {
    if let Some(x) = GLOBAL_ENV.get() {
        x
    } else if cfg!(debug_assertions) {
        panic!("Uninitalized static: GLOBAL_ENV")
    } else {
        unreachable!("Never forget to intialize before using it!")
    }
}

pub fn initialize_global(msg: Message) {
    let is_nvim = msg
        .params
        .get("is_nvim")
        .and_then(|x| x.as_bool())
        .unwrap_or(false);

    let enable_icon = msg
        .params
        .get("enable_icon")
        .and_then(|x| x.as_bool())
        .unwrap_or(false);

    let preview_size = msg
        .params
        .get("clap_preview_size")
        .expect("Missing clap_preview_size on initialize_global_env");

    let global_env = GlobalEnv::new(is_nvim, enable_icon, preview_size.clone());

    if let Err(e) = GLOBAL_ENV.set(global_env) {
        debug!("failed to initialized GLOBAL_ENV, error: {:?}", e);
    } else {
        debug!("GLOBAL_ENV initialized successfully");
    }
}
