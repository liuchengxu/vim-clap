use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::SystemTime;

use anyhow::Result;

use crate::error::DummyError;
use crate::icon::{prepend_grep_icon, prepend_icon, DEFAULT_ICONIZED};

/// Remove the last element if it's empty string.
#[inline]
fn trim_trailing(lines: &mut Vec<String>) {
    if let Some(last_line) = lines.last() {
        if last_line.is_empty() || last_line == DEFAULT_ICONIZED {
            lines.remove(lines.len() - 1);
        }
    }
}

pub fn set_current_dir(cmd: &mut Command, cmd_dir: Option<PathBuf>) {
    if let Some(cmd_dir) = cmd_dir {
        // If cmd_dir is not a directory, use its parent as current dir.
        if cmd_dir.is_dir() {
            cmd.current_dir(cmd_dir);
        } else {
            let mut cmd_dir = cmd_dir;
            cmd_dir.pop();
            cmd.current_dir(cmd_dir);
        }
    }
}

#[derive(Debug)]
pub struct LightCommand<'a> {
    cmd: &'a mut Command,
    total: usize,
    number: Option<usize>,
    output: Option<String>,
    enable_icon: bool,
    grep_enable_icon: bool,
    output_threshold: usize,
}

impl<'a> LightCommand<'a> {
    pub fn new(
        cmd: &'a mut Command,
        number: Option<usize>,
        output: Option<String>,
        enable_icon: bool,
        grep_enable_icon: bool,
        output_threshold: usize,
    ) -> Self {
        Self {
            cmd,
            number,
            total: 0usize,
            output,
            enable_icon,
            grep_enable_icon,
            output_threshold,
        }
    }

    pub fn new_grep(cmd: &'a mut Command, number: Option<usize>, grep_enable_icon: bool) -> Self {
        Self {
            cmd,
            number,
            total: 0usize,
            output: None,
            enable_icon: false,
            grep_enable_icon,
            output_threshold: 0usize,
        }
    }

    /// Collect the output of command, exit directly if any error happened.
    fn output(&mut self) -> Result<Output> {
        let cmd_output = self.cmd.output()?;

        // vim-clap does not handle the stderr stream, we just pass the error info via stdout.
        if !cmd_output.status.success() && !cmd_output.stderr.is_empty() {
            let error = format!("{}", String::from_utf8_lossy(&cmd_output.stderr));
            println_json!(error);
            std::process::exit(1);
        }

        Ok(cmd_output)
    }

    /// Normally we only care about the top N items and number of total results.
    fn minimalize_job_overhead(&self, stdout: &[u8]) -> Result<()> {
        if let Some(number) = self.number {
            // TODO: do not have to into String for whole stdout, find the nth index of newline.
            // &cmd_output.stdout[..nth_newline_index]
            let stdout_str = String::from_utf8_lossy(&stdout);
            let lines = self.try_prepend_icon(stdout_str.split('\n').take(number));
            let total = self.total;
            println_json!(total, lines);
            return Ok(());
        }
        Err(anyhow::Error::new(DummyError).context("No truncation"))
    }

    fn try_prepend_icon<'b>(&self, top_n: impl std::iter::Iterator<Item = &'b str>) -> Vec<String> {
        let mut lines = if self.grep_enable_icon {
            top_n.map(prepend_grep_icon).collect::<Vec<_>>()
        } else if self.enable_icon {
            top_n.map(prepend_icon).collect::<Vec<_>>()
        } else {
            top_n.map(Into::into).collect::<Vec<_>>()
        };
        trim_trailing(&mut lines);
        lines
    }

    fn tempfile(&self, args: &[String]) -> Result<PathBuf> {
        if let Some(ref output) = self.output {
            Ok(output.into())
        } else {
            let mut dir = std::env::temp_dir();
            dir.push(format!(
                "{}_{}",
                args.join("_"),
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)?
                    .as_secs()
            ));
            Ok(dir)
        }
    }

    /// Cache the stdout into a tempfile if the output threshold exceeds.
    fn try_cache(&self, cmd_stdout: &[u8], args: &[String]) -> Result<(String, Option<PathBuf>)> {
        if self.total > self.output_threshold {
            let tempfile = self.tempfile(args)?;
            File::create(&tempfile)?.write_all(cmd_stdout)?;
            // FIXME find the nth newline index of stdout.
            // let _end = std::cmp::min(cmd_stdout.len(), 500);
            Ok((
                // lines used for displaying directly.
                // &cmd_output.stdout[..nth_newline_index]
                String::from_utf8_lossy(cmd_stdout).into(),
                Some(tempfile),
            ))
        } else {
            Ok((String::from_utf8_lossy(cmd_stdout).into(), None))
        }
    }

    pub fn execute(&mut self, args: &[String]) -> Result<()> {
        let cmd_output = self.output()?;
        let cmd_stdout = &cmd_output.stdout;

        self.total = bytecount::count(cmd_stdout, b'\n');

        if self.minimalize_job_overhead(cmd_stdout).is_ok() {
            return Ok(());
        }

        // Write the output to a tempfile if the lines are too many.
        let (stdout_str, tempfile) = self.try_cache(&cmd_stdout, args)?;
        let lines = self.try_prepend_icon(stdout_str.split('\n'));
        let total = self.total;
        if let Some(tempfile) = tempfile {
            println_json!(total, lines, tempfile);
        } else {
            println_json!(total, lines);
        }

        Ok(())
    }
}
