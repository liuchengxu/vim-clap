use std::io::{BufRead, Lines};
use subprocess::Exec;

#[inline]
pub fn exec(cmd: Exec) -> std::io::Result<Lines<impl BufRead>> {
    // We usually have a decent amount of RAM nowadays.
    Ok(std::io::BufReader::with_capacity(
        8 * 1024 * 1024,
        cmd.stream_stdout()
            .map_err(|e| std::io::Error::other(e.to_string()))?,
    )
    .lines())
}
