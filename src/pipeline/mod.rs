pub mod analyze;
pub mod process;
pub mod visualize;

use anyhow::{Context, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::time::Duration;

use crate::cli::{AnalyzeArgs, MasterArgs};
use crate::config;
use crate::ffmpeg::Ffmpeg;
use crate::report;

// ── master ────────────────────────────────────────────────────────────────────

pub fn run_master(args: MasterArgs) -> anyhow::Result<()> {
    // Validate input
    if !args.input.exists() {
        anyhow::bail!("Input file not found: {}", args.input.display());
    }

    // Load preset and apply CLI overrides
    let mut preset = config::load_preset(&args.preset)
        .with_context(|| format!("Failed to load preset '{}'", args.preset))?;

    if let Some(lufs)      = args.lufs       { preset.target.lufs      = lufs; }
    if let Some(true_peak) = args.true_peak  { preset.target.true_peak = true_peak; }

    // Resolve output directory
    let out_dir = args.output.clone().unwrap_or_else(|| PathBuf::from("mastered"));
    std::fs::create_dir_all(&out_dir)?;

    // Locate ffmpeg
    let ffmpeg = Ffmpeg::find()?;

    println!("{} {}", "▸ preset:".bold(), preset.meta.name.cyan());
    println!("{} {}", "▸ target:".bold(),
        format!("{} LUFS  {} dBTP  {} LU LRA",
            preset.target.lufs, preset.target.true_peak, preset.target.lra).cyan()
    );
    println!();

    // Step 1: Pre-analysis
    let step = spinner("Analyzing input…");
    let (pre_analysis, measured) =
        analyze::analyze(&ffmpeg, &args.input, args.verbose)?;
    step.finish_and_clear();
    print_analysis_summary("pre-master", &pre_analysis);

    // Step 2: Pre-master visualization
    if !args.no_visualize && preset.visualization.enabled {
        let step = spinner("Generating pre-master visuals…");
        visualize::generate(
            &ffmpeg, &args.input, &out_dir, &preset.visualization, "pre", args.verbose,
        )?;
        step.finish_and_clear();
        println!("  {} pre-master visuals written to {}", "✓".green(), out_dir.display());
    }

    if args.dry_run {
        println!();
        println!("{}", "dry-run mode — no output file written.".yellow());
        return Ok(());
    }

    // Step 3: Derive output filename
    let stem = args.input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let ext = match preset.output.format.as_str() {
        "flac" => "flac",
        "mp3"  => "mp3",
        "aac"  => "m4a",
        _      => "wav",
    };
    let out_name = format!("{}{}.{}", stem, args.suffix, ext);
    let out_path = out_dir.join(&out_name);

    // Step 4: Process
    let step = spinner("Mastering…");
    let result = process::run_process(
        &ffmpeg, &args.input, &out_path, &preset, &measured, args.verbose,
    )?;
    step.finish_and_clear();
    println!("  {} mastered: {}", "✓".green(), out_path.display());

    // Step 5: Post-analysis
    let step = spinner("Verifying output…");
    let (post_analysis, _) = analyze::analyze(&ffmpeg, &out_path, false)?;
    step.finish_and_clear();
    print_analysis_summary("post-master", &post_analysis);

    // Step 6: Post-master visualization
    if !args.no_visualize && preset.visualization.enabled {
        let step = spinner("Generating post-master visuals…");
        visualize::generate(
            &ffmpeg, &out_path, &out_dir, &preset.visualization, "post", args.verbose,
        )?;
        step.finish_and_clear();
        println!("  {} post-master visuals written to {}", "✓".green(), out_dir.display());
    }

    // Step 7: Report
    let report_path = out_dir.join(format!("{}{}_report.json", stem, args.suffix));
    report::write_json(
        &report_path, &args.input, &out_path, &preset, &pre_analysis, &post_analysis,
        &result.filter_chain,
    )?;
    println!("  {} report: {}", "✓".green(), report_path.display());

    println!();
    println!("{}", "done.".bold().green());
    Ok(())
}

// ── analyze ───────────────────────────────────────────────────────────────────

pub fn run_analyze(args: AnalyzeArgs) -> anyhow::Result<()> {
    if !args.input.exists() {
        anyhow::bail!("Input file not found: {}", args.input.display());
    }

    let ffmpeg = Ffmpeg::find()?;

    let step = spinner("Analyzing…");
    let (analysis, _) = analyze::analyze(&ffmpeg, &args.input, args.verbose)?;
    step.finish_and_clear();

    print_analysis_summary("analysis", &analysis);

    if args.visualize {
        let out_dir = args.output.unwrap_or_else(|| PathBuf::from("."));
        let cfg = config::Preset::default().visualization;
        let step = spinner("Generating visuals…");
        let paths = visualize::generate(&ffmpeg, &args.input, &out_dir, &cfg, "viz", args.verbose)?;
        step.finish_and_clear();

        if let Some(p) = paths.spectrogram {
            println!("  {} spectrogram: {}", "✓".green(), p.display());
        }
        if let Some(p) = paths.waveform {
            println!("  {} waveform:    {}", "✓".green(), p.display());
        }
    }

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("  {spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠚", "⠞", "⠖", "⠦", "⠴", "⠲", "⠳", "⠓"]),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

fn print_analysis_summary(label: &str, a: &analyze::AudioAnalysis) {
    println!("  {}", format!("[{label}]").bold());
    println!(
        "    {:22} {}",
        "integrated loudness".dimmed(),
        format!("{:.1} LUFS", a.integrated_lufs).cyan()
    );
    println!(
        "    {:22} {}",
        "true peak".dimmed(),
        format!("{:.1} dBTP", a.true_peak_dbtp).cyan()
    );
    println!(
        "    {:22} {}",
        "loudness range".dimmed(),
        format!("{:.1} LU", a.loudness_range_lu).cyan()
    );
    println!(
        "    {:22} {}",
        "duration".dimmed(),
        a.duration_display().cyan()
    );
    println!(
        "    {:22} {}",
        "format".dimmed(),
        format!("{} / {} Hz / {} ch", a.codec, a.sample_rate, a.channels).cyan()
    );
    println!();
}
