use clap::Parser;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Name of the person to greet
    #[arg(short, long, default_value = "./.befating/befaring.toml")]
    pub path: String,

    /// App stdout and stderr at the end
    #[arg(short = 'o', long)]
    pub app_output: bool,
}
