use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;

// The output is wrapped in a Result to allow matching on errors
// Returns an Iterator to the Reader of the lines of the file.
pub fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

/// Returns the first number lines given the file path.
pub fn read_first_lines<P: AsRef<Path>>(
    filename: P,
    number: usize,
) -> io::Result<impl Iterator<Item = String>> {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file)
        .lines()
        .filter_map(|i| i.ok())
        .take(number))
}
