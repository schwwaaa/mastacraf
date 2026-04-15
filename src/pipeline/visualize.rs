use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::config::VisualizationConfig;
use crate::ffmpeg::Ffmpeg;

pub struct VisualPaths {
    pub spectrogram: Option<PathBuf>,
    pub waveform:    Option<PathBuf>,
}

/// Generate spectrogram and/or waveform images for an audio file.
///
/// `tag` is appended to the filename stem so you can distinguish pre/post:
///   "pre"  → track_pre_spectrogram.png
///   "post" → track_post_spectrogram.png
pub fn generate(
    ffmpeg:    &Ffmpeg,
    input:     &Path,
    out_dir:   &Path,
    cfg:       &VisualizationConfig,
    tag:       &str,
    verbose:   bool,
) -> Result<VisualPaths> {
    if !cfg.enabled {
        return Ok(VisualPaths { spectrogram: None, waveform: None });
    }

    std::fs::create_dir_all(out_dir)?;

    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("audio");

    let input_str = input.to_str().unwrap_or("");

    // ── Spectrogram ──────────────────────────────────────────────────────────
    let spectrogram_path = if cfg.spectrogram {
        let path = out_dir.join(format!("{stem}_{tag}_spectrogram.png"));
        let size  = format!("{}x{}", cfg.width, cfg.spectrogram_height);

        // showspectrumpic renders the whole file as a single PNG image.
        // color=intensity gives a heatmap feel; scale=log is perceptually useful
        // for wide-range content.
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

    // ── Waveform ─────────────────────────────────────────────────────────────
    let waveform_path = if cfg.waveform {
        let path = out_dir.join(format!("{stem}_{tag}_waveform.png"));
        let size  = format!("{}x{}", cfg.width, cfg.waveform_height);

        // showwavespic renders a static waveform overview.
        // split_channels=1 draws L/R separately so stereo content is readable.
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
