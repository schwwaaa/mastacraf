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
    if !args.input.exists() {
        anyhow::bail!("Input file not found: {}", args.input.display());
    }

    let mut preset = config::load_preset(&args.preset)
        .with_context(|| format!("Failed to load preset '{}'", args.preset))?;

    if let Some(lufs)      = args.lufs      { preset.target.lufs      = lufs; }
    if let Some(true_peak) = args.true_peak { preset.target.true_peak = true_peak; }

    let stem = args.input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output")
        .to_string();

    let base_dir = args.output.clone().unwrap_or_else(|| PathBuf::from("mastered"));
    let out_dir  = base_dir.join(&stem);
    std::fs::create_dir_all(&out_dir)?;

    let ffmpeg = Ffmpeg::find()?;

    println!("{} {}", "▸ input:".bold(),  args.input.display().to_string().cyan());
    println!("{} {}", "▸ output:".bold(), out_dir.display().to_string().cyan());
    println!("{} {}", "▸ preset:".bold(), preset.meta.name.cyan());
    println!(
        "{} {}",
        "▸ target:".bold(),
        format!("{} LUFS  {} dBTP  {} LU LRA",
            preset.target.lufs, preset.target.true_peak, preset.target.lra).cyan()
    );
    println!();

    // Step 1: Pre-analysis
    let step = spinner("Analyzing input…");
    let (pre_analysis, measured) = analyze::analyze(&ffmpeg, &args.input, args.verbose)?;
    step.finish_and_clear();
    print_analysis_summary("pre-master", &pre_analysis);

    // Step 2: Pre-master visualization
    let pre_paths = if !args.no_visualize && preset.visualization.enabled {
        let step = spinner("Generating pre-master visuals…");
        let paths = visualize::generate(
            &ffmpeg, &args.input, &out_dir, &preset.visualization, "pre", args.verbose,
        )?;
        step.finish_and_clear();
        println!("  {} pre_spectrogram.png  pre_waveform.png", "✓".green());
        paths
    } else {
        visualize::VisualPaths { spectrogram: None, waveform: None }
    };

    if args.dry_run {
        println!();
        println!("{}", "dry-run mode — no output file written.".yellow());
        return Ok(());
    }

    // Step 3: Output filename
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
    println!("  {} mastered: {}", "✓".green(), out_path.file_name().unwrap_or_default().to_string_lossy());

    // Step 5: Post-analysis
    let step = spinner("Verifying output…");
    let (post_analysis, _) = analyze::analyze(&ffmpeg, &out_path, false)?;
    step.finish_and_clear();
    print_analysis_summary("post-master", &post_analysis);

    // Step 6: Post-master visualization
    let post_paths = if !args.no_visualize && preset.visualization.enabled {
        let step = spinner("Generating post-master visuals…");
        let paths = visualize::generate(
            &ffmpeg, &out_path, &out_dir, &preset.visualization, "post", args.verbose,
        )?;
        step.finish_and_clear();
        println!("  {} post_spectrogram.png  post_waveform.png", "✓".green());
        paths
    } else {
        visualize::VisualPaths { spectrogram: None, waveform: None }
    };

    // Step 7: Comparison visuals — stacked and diff
    if !args.no_visualize && preset.visualization.enabled {
        let step = spinner("Generating comparison visuals…");
        let cmp = visualize::generate_comparisons(
            &ffmpeg, &pre_paths, &post_paths, &out_dir, &stem, args.verbose,
        )?;
        step.finish_and_clear();

        let mut generated: Vec<&str> = vec![];
        if cmp.stacked_spectrogram.is_some() { generated.push("compare_spectrogram.png"); }
        if cmp.stacked_waveform.is_some()    { generated.push("compare_waveform.png"); }
        if cmp.diff_spectrogram.is_some()    { generated.push("diff_spectrogram.png"); }
        if cmp.diff_waveform.is_some()       { generated.push("diff_waveform.png"); }

        if !generated.is_empty() {
            println!("  {} {}", "✓".green(), generated.join("  "));
        }
    }

    // Step 8: Report
    let report_path = out_dir.join(format!("{}{}_report.json", stem, args.suffix));
    report::write_json(
        &report_path, &args.input, &out_path, &preset,
        &pre_analysis, &post_analysis, &result.filter_chain,
    )?;
    println!("  {} report: {}", "✓".green(), report_path.file_name().unwrap_or_default().to_string_lossy());

    // Summary
    println!();
    println!("{}", "Output files:".bold().dimmed());
    println!("  {}", out_dir.display().to_string().dimmed());
    println!();
    println!("{}", format!("done.").bold().green());
    Ok(())
}

// ── analyze command ───────────────────────────────────────────────────────────

pub fn run_analyze(args: AnalyzeArgs) -> anyhow::Result<()> {
    if !args.input.exists() {
        anyhow::bail!("Input file not found: {}", args.input.display());
    }

    let ffmpeg = Ffmpeg::find()?;

    let step = spinner("Analyzing…");
    let (analysis, _) = analyze::analyze(&ffmpeg, &args.input, args.verbose)?;
    step.finish_and_clear();

    print_analysis_summary("analysis", &analysis);
    print_extended_summary(&analysis);

    if args.visualize {
        let out_dir = args.output.unwrap_or_else(|| PathBuf::from("."));
        let cfg     = config::Preset::default().visualization;
        let step    = spinner("Generating visuals…");
        let paths   = visualize::generate(
            &ffmpeg, &args.input, &out_dir, &cfg, "viz", args.verbose,
        )?;
        step.finish_and_clear();

        if let Some(p) = paths.spectrogram { println!("  {} spectrogram: {}", "✓".green(), p.display()); }
        if let Some(p) = paths.waveform    { println!("  {} waveform:    {}", "✓".green(), p.display()); }
    }

    Ok(())
}

// ── Console output ────────────────────────────────────────────────────────────

fn spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("  {spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋","⠙","⠚","⠞","⠖","⠦","⠴","⠲","⠳","⠓"]),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

fn print_analysis_summary(label: &str, a: &analyze::AudioAnalysis) {
    println!("  {}", format!("[{label}]").bold());
    println!("    {:26} {}", "integrated loudness".dimmed(), format!("{:.1} LUFS", a.integrated_lufs).cyan());
    println!("    {:26} {}", "true peak".dimmed(),           format!("{:.1} dBTP", a.true_peak_dbtp).cyan());
    println!("    {:26} {}", "loudness range".dimmed(),      format!("{:.1} LU",   a.loudness_range_lu).cyan());
    println!("    {:26} {}", "duration".dimmed(),            a.duration_display().cyan());
    println!("    {:26} {}", "format".dimmed(),
        format!("{} / {} Hz / {} ch", a.codec, a.sample_rate, a.channels).cyan()
    );
    println!();
}

fn print_extended_summary(a: &analyze::AudioAnalysis) {
    let Some(ref ext) = a.extended else { return };

    println!("  {}", "[extended analysis]".bold());
    println!("    {:26} {}", "RMS level".dimmed(),
        format!("{:.1} dBFS", ext.rms_dbfs).cyan());
    println!("    {:26} {}", "crest factor".dimmed(),
        format!("{:.1} dB",   ext.crest_factor_db).cyan());
    println!("    {:26} {}", "dynamic range (DR)".dimmed(),
        format!("DR{:.0}",    ext.dynamic_range_dr).cyan());
    println!("    {:26} {}", "DC offset".dimmed(),
        format!("{:.5}",       ext.dc_offset).cyan());

    if let Some(corr) = ext.phase_correlation {
        println!("    {:26} {}  ({})", "phase correlation".dimmed(),
            format!("{:.3}", corr).cyan(), phase_label(corr));
    }

    println!("    {:26} low {:.0}%  mid {:.0}%  high {:.0}%",
        "spectral balance".dimmed(),
        ext.spectral_balance.low_pct,
        ext.spectral_balance.mid_pct,
        ext.spectral_balance.high_pct,
    );
    println!("    {:26} {} Hz", "spectral centroid".dimmed(),
        format!("{:.0}", ext.spectral_centroid_hz).cyan());
    println!();
}

fn phase_label(corr: f32) -> colored::ColoredString {
    if      corr > 0.8  { "in phase / wide stereo".green() }
    else if corr > 0.3  { "normal stereo".cyan() }
    else if corr > -0.2 { "narrow or out-of-phase — check".yellow() }
    else                { "significant phase problem".red() }
}
