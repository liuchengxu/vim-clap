use std::sync::Arc;

use anyhow::Result;
use crossbeam_channel::Receiver;
use log::{debug, error};
use parking_lot::Mutex;

use crate::stdio_server::state::State;
use crate::stdio_server::types::Call;

#[derive(Clone)]
pub struct SessionClient {
    pub state_mutex: Arc<Mutex<State>>,
}

impl SessionClient {
    pub fn new(state: State) -> Self {
        Self {
            state_mutex: Arc::new(Mutex::new(state)),
        }
    }

    pub fn loop_call(&self, rx: &Receiver<Call>) {
        for call in rx.iter() {
            let session_client = self.clone();
            tokio::spawn(async move {
                if let Err(e) = session_client.handle_call(call).await {
                    error!("Error handling request: {:?}", e);
                }
            });
        }
    }

    pub async fn handle_call(self, call: Call) -> Result<()> {
        match call {
            Call::Notification(notification) => {
                tokio::spawn(async move {
                    if let Err(e) = notification.handle().await {
                        error!("Error occurred when handling notification: {:?}", e)
                    }
                });
            }
            Call::MethodCall(method_call) => {
                let id = method_call.id;
                let result = method_call.handle().await;
                let state = self.state_mutex.lock();
                state.vim.rpc_client.output(id, result)?;
            }
        }
        Ok(())
    }
}
