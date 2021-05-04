pub mod light;
pub mod std;
pub mod tokio;

use anyhow::Result;

use self::std::StdCommand;

pub fn process_output(output: ::std::process::Output) -> Result<Vec<String>> {
    if !output.status.success() && !output.stderr.is_empty() {
        return Err(anyhow::anyhow!("an error occured: {:?}", output.stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut output_lines = stdout.split('\n').map(Into::into).collect::<Vec<String>>();

    // Remove the last empty line.
    if output_lines.last().map(|s| s.is_empty()).unwrap_or(false) {
        output_lines.pop();
    }

    Ok(output_lines)
}

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
