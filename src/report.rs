use anyhow::Result;
use chrono::Utc;
use serde::Serialize;
use std::path::Path;

use crate::config::Preset;
use crate::pipeline::analyze::AudioAnalysis;

#[derive(Serialize)]
struct MasteringReport<'a> {
    generated_at:  String,
    input_file:    String,
    output_file:   String,
    preset:        &'a Preset,
    pre_analysis:  &'a AudioAnalysis,
    post_analysis: &'a AudioAnalysis,
    filter_chain:  &'a str,
    delta: Delta,
}

#[derive(Serialize)]
struct Delta {
    lufs_change:      f32,
    true_peak_change: f32,
    lra_change:       f32,
}

pub fn write_json(
    path:          &Path,
    input:         &Path,
    output:        &Path,
    preset:        &Preset,
    pre:           &AudioAnalysis,
    post:          &AudioAnalysis,
    filter_chain:  &str,
) -> Result<()> {
    let report = MasteringReport {
        generated_at:  Utc::now().to_rfc3339(),
        input_file:    input.display().to_string(),
        output_file:   output.display().to_string(),
        preset,
        pre_analysis:  pre,
        post_analysis: post,
        filter_chain,
        delta: Delta {
            lufs_change:      post.integrated_lufs  - pre.integrated_lufs,
            true_peak_change: post.true_peak_dbtp   - pre.true_peak_dbtp,
            lra_change:       post.loudness_range_lu - pre.loudness_range_lu,
        },
    };

    let json = serde_json::to_string_pretty(&report)?;
    std::fs::write(path, json)?;
    Ok(())
}
