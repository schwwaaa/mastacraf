use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::config::VisualizationConfig;
use crate::ffmpeg::Ffmpeg;

// ── Output path structs ───────────────────────────────────────────────────────

pub struct VisualPaths {
    pub spectrogram: Option<PathBuf>,
    pub waveform:    Option<PathBuf>,
}

/// All comparison outputs produced after both pre and post visuals exist.
pub struct ComparisonPaths {
    /// Pre (top) and post (bottom) spectrogram stacked with an amber separator.
    pub stacked_spectrogram: Option<PathBuf>,
    /// Pre (top) and post (bottom) waveform stacked with an amber separator.
    pub stacked_waveform:    Option<PathBuf>,
    /// Pixel-difference between pre and post spectrogram, contrast-amplified.
    /// Dark = no change. Bright saturated color = significant spectral shift.
    pub diff_spectrogram:    Option<PathBuf>,
    /// Pixel-difference between pre and post waveform.
    pub diff_waveform:       Option<PathBuf>,
}

// ── Per-pass generation ───────────────────────────────────────────────────────

/// Generate spectrogram and waveform for one audio file.
/// `tag` is appended to the stem: "pre" → `<stem>_pre_spectrogram.png`.
pub fn generate(
    ffmpeg:  &Ffmpeg,
    input:   &Path,
    out_dir: &Path,
    cfg:     &VisualizationConfig,
    tag:     &str,
    verbose: bool,
) -> Result<VisualPaths> {
    if !cfg.enabled {
        return Ok(VisualPaths { spectrogram: None, waveform: None });
    }

    std::fs::create_dir_all(out_dir)?;

    let stem      = input.file_stem().and_then(|s| s.to_str()).unwrap_or("audio");
    let input_str = input.to_str().unwrap_or("");

    // ── Spectrogram ───────────────────────────────────────────────────────────
    let spectrogram_path = if cfg.spectrogram {
        let path   = out_dir.join(format!("{stem}_{tag}_spectrogram.png"));
        let size   = format!("{}x{}", cfg.width, cfg.spectrogram_height);
        let filter = format!(
            "showspectrumpic=s={size}:mode=combined:color=intensity:scale=log:legend=1"
        );
        ffmpeg.run_verbose(
            &["-hide_banner", "-i", input_str, "-lavfi", &filter, path.to_str().unwrap_or("")],
            verbose,
        )?;
        Some(path)
    } else {
        None
    };

    // ── Waveform ──────────────────────────────────────────────────────────────
    let waveform_path = if cfg.waveform {
        let path   = out_dir.join(format!("{stem}_{tag}_waveform.png"));
        let size   = format!("{}x{}", cfg.width, cfg.waveform_height);
        let filter = format!("showwavespic=s={size}:split_channels=1:colors=0x4EA8DE|0xFF8C42");
        ffmpeg.run_verbose(
            &["-hide_banner", "-i", input_str, "-lavfi", &filter, path.to_str().unwrap_or("")],
            verbose,
        )?;
        Some(path)
    } else {
        None
    };

    Ok(VisualPaths { spectrogram: spectrogram_path, waveform: waveform_path })
}

// ── Comparison generation ─────────────────────────────────────────────────────

/// Generate stacked and diff comparison images from pre and post visual paths.
///
/// Stacked  — pre image on top, 3px amber (#e8913a) separator, post below.
///            Gives an at-a-glance before/after aligned on the same time axis.
///
/// Diff     — absolute pixel difference between pre and post, with aggressive
///            contrast amplification and saturation boost. The result:
///              · Black / very dark  = no change between pre and post
///              · Bright/saturated   = significant spectral or amplitude change
///            Because spectrograms use hue to represent frequency energy, the diff
///            image preserves those hue channels in the changed regions, making
///            it easy to see not just *where* things changed but *what kind* of
///            change (e.g. a yellow-shifted region means the high-frequency content
///            increased; a blue-shifted region means it decreased).
pub fn generate_comparisons(
    ffmpeg:  &Ffmpeg,
    pre:     &VisualPaths,
    post:    &VisualPaths,
    out_dir: &Path,
    stem:    &str,
    verbose: bool,
) -> Result<ComparisonPaths> {
    let stacked_spec = pair_op(&pre.spectrogram, &post.spectrogram, |pre_p, post_p| {
        let out = out_dir.join(format!("{stem}_compare_spectrogram.png"));
        generate_stacked(ffmpeg, pre_p, post_p, &out, verbose)?;
        Ok(out)
    });

    let stacked_wav = pair_op(&pre.waveform, &post.waveform, |pre_p, post_p| {
        let out = out_dir.join(format!("{stem}_compare_waveform.png"));
        generate_stacked(ffmpeg, pre_p, post_p, &out, verbose)?;
        Ok(out)
    });

    let diff_spec = pair_op(&pre.spectrogram, &post.spectrogram, |pre_p, post_p| {
        let out = out_dir.join(format!("{stem}_diff_spectrogram.png"));
        generate_diff(ffmpeg, pre_p, post_p, &out, verbose)?;
        Ok(out)
    });

    let diff_wav = pair_op(&pre.waveform, &post.waveform, |pre_p, post_p| {
        let out = out_dir.join(format!("{stem}_diff_waveform.png"));
        generate_diff(ffmpeg, pre_p, post_p, &out, verbose)?;
        Ok(out)
    });

    Ok(ComparisonPaths {
        stacked_spectrogram: stacked_spec,
        stacked_waveform:    stacked_wav,
        diff_spectrogram:    diff_spec,
        diff_waveform:       diff_wav,
    })
}

// ── Stacked ───────────────────────────────────────────────────────────────────

fn generate_stacked(
    ffmpeg:  &Ffmpeg,
    top:     &Path,
    bottom:  &Path,
    out:     &Path,
    verbose: bool,
) -> Result<()> {
    // Add a 3px amber separator to the bottom edge of the top image, then vstack.
    // This creates a clear visual boundary between pre and post without needing
    // drawtext (which requires a freetype-enabled FFmpeg build).
    let filter_complex =
        "[0:v]pad=iw:ih+3:0:0:color=0xe8913a[top];\
         [top][1:v]vstack=inputs=2[out]";

    ffmpeg.run_verbose(
        &[
            "-hide_banner",
            "-i", top.to_str().unwrap_or(""),
            "-i", bottom.to_str().unwrap_or(""),
            "-filter_complex", filter_complex,
            "-map", "[out]",
            "-y",
            out.to_str().unwrap_or(""),
        ],
        verbose,
    )?;
    Ok(())
}

// ── Diff ──────────────────────────────────────────────────────────────────────

fn generate_diff(
    ffmpeg:  &Ffmpeg,
    pre:     &Path,
    post:    &Path,
    out:     &Path,
    verbose: bool,
) -> Result<()> {
    // Pipeline:
    //   1. blend=difference   — absolute per-pixel difference of pre and post
    //   2. curves             — aggressive contrast expansion:
    //                           anything above ~12% of max difference maps to
    //                           full brightness. This makes even subtle changes
    //                           visible while keeping unchanged regions black.
    //   3. hue=s=6            — extreme saturation boost so the existing color
    //                           channels (which carry frequency-region information
    //                           from the original spectrograms) become vivid.
    //                           Result: hue encodes *what changed*, brightness
    //                           encodes *how much it changed*.
    //
    // For waveforms (mostly single-color images), the diff will appear as
    // a bright amplitude-difference map — regions where the envelope changed
    // appear as bright lines on a black background.
    let filter_complex =
        "[0:v][1:v]blend=all_mode=difference[diff];\
         [diff]curves=all='0/0 0.12/1 1/1',hue=s=6[out]";

    ffmpeg.run_verbose(
        &[
            "-hide_banner",
            "-i", pre.to_str().unwrap_or(""),
            "-i", post.to_str().unwrap_or(""),
            "-filter_complex", filter_complex,
            "-map", "[out]",
            "-y",
            out.to_str().unwrap_or(""),
        ],
        verbose,
    )?;
    Ok(())
}

// ── Utility ───────────────────────────────────────────────────────────────────

/// Apply a function to two Option<PathBuf> values if both are Some.
/// Returns None if either is None or if the function returns an error (non-fatal).
fn pair_op<F>(a: &Option<PathBuf>, b: &Option<PathBuf>, f: F) -> Option<PathBuf>
where
    F: FnOnce(&Path, &Path) -> Result<PathBuf>,
{
    match (a, b) {
        (Some(pa), Some(pb)) => f(pa, pb).ok(),
        _ => None,
    }
}
