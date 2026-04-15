use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use colored::Colorize;
use std::path::PathBuf;

use crate::{config, pipeline};

#[derive(Parser)]
#[command(
    name = "mastercraft",
    version,
    about = "Custom audio mastering pipeline for experimental electronic music"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

impl Cli {
    pub fn execute(self) -> Result<()> {
        print_banner();
        match self.command {
            Commands::Master(args)    => pipeline::run_master(args),
            Commands::Analyze(args)   => pipeline::run_analyze(args),
            Commands::Presets         => run_list_presets(),
            Commands::Preset { name } => run_show_preset(&name),
        }
    }
}

fn print_banner() {
    println!("{}", "mastercraft".bold().cyan());
    println!("{}", "audio mastering pipeline for experimental music".dimmed());
    println!();
}

#[derive(Subcommand)]
pub enum Commands {
    /// Master an audio file through the full pipeline
    Master(MasterArgs),
    /// Analyze an audio file without mastering
    Analyze(AnalyzeArgs),
    /// List available presets
    Presets,
    /// Show full details of a named preset
    Preset { name: String },
}

#[derive(Args)]
pub struct MasterArgs {
    /// Input audio file (WAV, FLAC, AIFF, etc.)
    pub input: PathBuf,

    /// Output directory  [default: ./mastered/]
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Mastering preset to use
    #[arg(short, long, default_value = "default")]
    pub preset: String,

    /// Disable visualization generation
    #[arg(long)]
    pub no_visualize: bool,

    /// Analyze only — do not write a mastered file
    #[arg(long)]
    pub dry_run: bool,

    /// Override integrated loudness target (LUFS)
    #[arg(long)]
    pub lufs: Option<f32>,

    /// Override true peak ceiling (dBTP)
    #[arg(long)]
    pub true_peak: Option<f32>,

    /// Filename suffix appended to the stem
    #[arg(long, default_value = "_master")]
    pub suffix: String,

    /// Print ffmpeg commands as they run
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Args)]
pub struct AnalyzeArgs {
    /// Input audio file
    pub input: PathBuf,

    /// Generate spectrogram and waveform images
    #[arg(long)]
    pub visualize: bool,

    /// Output directory for visualizations
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Print ffmpeg commands as they run
    #[arg(short, long)]
    pub verbose: bool,
}

fn run_list_presets() -> Result<()> {
    let presets = config::list_presets()?;

    println!("{}", "available presets".bold());
    println!();
    println!(
        "  {:20} {:8} {:8} {:6}  {}",
        "name".bold(),
        "LUFS".bold(),
        "TP".bold(),
        "LRA".bold(),
        "description".bold()
    );
    println!("  {}", "-".repeat(72).dimmed());

    // Built-in default
    let def = config::Preset::default();
    println!(
        "  {:20} {:8} {:8} {:6}  {}",
        "default".green(),
        format!("{}", def.target.lufs),
        format!("{} dBTP", def.target.true_peak),
        format!("{} LU", def.target.lra),
        def.meta.description.dimmed()
    );

    for (name, _path) in &presets {
        if name == "default" {
            continue;
        }
        match config::load_preset(name) {
            Ok(p) => {
                println!(
                    "  {:20} {:8} {:8} {:6}  {}",
                    name.green(),
                    format!("{}", p.target.lufs),
                    format!("{} dBTP", p.target.true_peak),
                    format!("{} LU", p.target.lra),
                    p.meta.description.dimmed()
                );
            }
            Err(_) => {
                println!("  {:20} {}", name.yellow(), "(failed to parse)".red());
            }
        }
    }

    println!();
    println!(
        "{}",
        "Preset files live in ./presets/ (relative to your working directory).".dimmed()
    );
    Ok(())
}

fn run_show_preset(name: &str) -> Result<()> {
    let preset = config::load_preset(name)?;
    let toml_str = toml::to_string_pretty(&preset)?;
    println!("{}", toml_str);
    Ok(())
}
