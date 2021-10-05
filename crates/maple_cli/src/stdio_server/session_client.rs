use anyhow::Result;
use crossbeam_channel::Receiver;
use log::error;

use crate::stdio_server::types::Call;

#[derive(Clone)]
pub struct SessionClient;

impl SessionClient {
    pub fn loop_call(&self, rx: &Receiver<Call>) {
        for call in rx.iter() {
            let session_client = self.clone();
            std::thread::spawn(move || {
                if let Err(e) = session_client.handle_call(call) {
                    error!("Error handling request: {:?}", e);
                }
            });
        }
    }

    pub fn handle_call(self, call: Call) -> Result<()> {
        match call {
            Call::Notification(notification) => {
                tokio::spawn(async move {
                    if let Err(e) = notification.handle().await {
                        error!("Error occurred when handling notification: {:?}", e)
                    }
                });
            }
            Call::MethodCall(method_call) => {}
        }
        Ok(())
    }
}
