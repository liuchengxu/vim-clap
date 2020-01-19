use std::path::PathBuf;

use structopt::clap::arg_enum;
use structopt::StructOpt;

arg_enum! {
    #[derive(Debug)]
    pub enum Algo {
        Skim,
        Fzy,
    }
}

#[derive(StructOpt, Debug)]
pub enum Cmd {
    /// Fuzzy filter the input
    #[structopt(name = "filter")]
    Filter {
        /// Initial query string
        #[structopt(index = 1, short, long)]
        query: String,

        /// Filter algorithm
        #[structopt(short, long, possible_values = &Algo::variants(), case_insensitive = true)]
        algo: Option<Algo>,

        /// Read input from a file instead of stdin, only absolute file path is supported.
        #[structopt(long = "input", parse(from_os_str))]
        input: Option<PathBuf>,
    },
    /// Execute the command
    #[structopt(name = "exec")]
    Exec {
        /// Specify the system command to run.
        #[structopt(index = 1, short, long)]
        cmd: String,

        /// Specify the output file path when the output of command exceeds the threshold.
        #[structopt(long = "output")]
        output: Option<String>,

        /// Specify the threshold for writing the output of command to a tempfile.
        #[structopt(long = "output-threshold", default_value = "100000")]
        output_threshold: usize,

        /// Specify the working directory of CMD
        #[structopt(long = "cmd-dir", parse(from_os_str))]
        cmd_dir: Option<PathBuf>,
    },
    /// Execute the grep command to avoid the escape issue
    #[structopt(name = "grep")]
    Grep {
        /// Specify the grep command to run, normally rg will be used.
        ///
        /// Incase of clap can not reconginize such option: --cmd "rg --vimgrep ... "fn ul"".
        ///                                                       |-----------------|
        ///                                                   this can be seen as an option by mistake.
        #[structopt(index = 1, short, long)]
        grep_cmd: String,

        /// Specify the query string for GREP_CMD.
        #[structopt(index = 2, short, long)]
        grep_query: String,

        /// Specify the working directory of CMD
        #[structopt(long = "cmd-dir", parse(from_os_str))]
        cmd_dir: Option<PathBuf>,
    },
}
