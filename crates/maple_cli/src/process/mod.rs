pub mod light;
pub mod std;
pub mod tokio;

use anyhow::Result;

use self::std::StdCommand;

#[derive(Debug)]
pub struct AsyncCommand(pub StdCommand);

impl AsyncCommand {
    pub fn new(command: String) -> Self {
        Self(command.into())
    }

    pub async fn lines(&mut self) -> Result<Vec<String>> {
        self.0.lines().await
    }
}
