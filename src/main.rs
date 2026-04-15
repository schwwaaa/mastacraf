mod cli;
mod config;
mod ffmpeg;
mod pipeline;
mod report;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    cli::Cli::parse().execute()
}
