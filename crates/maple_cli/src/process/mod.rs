pub mod light;
pub mod std;
pub mod tokio;

use anyhow::Result;

use self::std::StdCommand;

/// Converts [`std::process::Output`] to a Vec of String.
///
/// Remove the last line if it's empty.
pub fn process_output(output: ::std::process::Output) -> Result<Vec<String>> {
    if !output.status.success() && !output.stderr.is_empty() {
        return Err(anyhow::anyhow!("an error occured: {:?}", output.stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut lines = stdout.split('\n').map(Into::into).collect::<Vec<String>>();

    // Remove the last empty line.
    if lines.last().map(|s| s.is_empty()).unwrap_or(false) {
        lines.pop();
    }

    Ok(lines)
}

/// Wrapper type of `StdCommand`.
#[derive(Debug)]
pub struct AsyncCommand(StdCommand);

impl AsyncCommand {
    pub fn new(command: String) -> Self {
        Self(command.into())
    }

    pub fn current_dir<P: AsRef<::std::path::Path>>(&mut self, dir: P) -> &mut Self {
        self.0.current_dir(dir);
        self
    }

    pub async fn lines(&mut self) -> Result<Vec<String>> {
        self.0.lines().await
    }
}
