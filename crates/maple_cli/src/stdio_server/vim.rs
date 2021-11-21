use std::sync::Arc;

use anyhow::Result;
use serde::de::DeserializeOwned;
use serde_json::json;

use crate::stdio_server::rpc::RpcClient;

#[derive(Clone)]
pub struct Vim {
    pub rpc_client: Arc<RpcClient>,
}

impl Vim {
    pub fn new(rpc_client: Arc<RpcClient>) -> Self {
        Self { rpc_client }
    }

    pub fn getbufvar<R: DeserializeOwned>(&self, bufname: &str, var: &str) -> Result<R> {
        self.rpc_client.call("getbufvar", json!([bufname, var]))
    }
}
