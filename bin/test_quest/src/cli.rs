use clap::Parser;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Name of the person to greet
    #[arg(short, long, default_value = "test_quest/test_quest.toml")]
    pub path: String,

    /// App stdout and stderr at the end
    #[arg(short = 'o', long)]
    pub app_output: bool,

    /// If this is sat by running --stream-app, the output from the application
    /// while be printed as it comes
    #[arg(long)]
    pub stream_app: bool,
}
