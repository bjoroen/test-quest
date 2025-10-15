use clap::Parser;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Name of the person to greet
    #[arg(short, long, default_value = "./proof.toml")]
    pub path: String,
}
