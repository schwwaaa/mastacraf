use anyhow::Result;
use std::path::Path;

use crate::config::Preset;
use crate::ffmpeg::Ffmpeg;
use super::analyze::Measured;

// ── Filter chain builder ──────────────────────────────────────────────────────

/// Build the complete ffmpeg audio filter chain from a preset + measured values.
///
/// Chain order:
///   highpass → [lowpass] → [compressor] → [limiter] → loudnorm (pass 2)
pub fn build_filter_chain(preset: &Preset, measured: &Measured) -> String {
    let mut filters: Vec<String> = vec![];

    // 1. High-pass — remove subsonic / DC
    if preset.filters.highpass_hz > 0.0 {
        filters.push(format!("highpass=f={:.1}", preset.filters.highpass_hz));
    }

    // 2. Low-pass (optional — rarely needed but available)
    if preset.filters.lowpass_hz > 0.0 {
        filters.push(format!("lowpass=f={:.1}", preset.filters.lowpass_hz));
    }

    // 3. Compressor (optional)
    if preset.compressor.enabled {
        let c = &preset.compressor;
        filters.push(format!(
            "acompressor=\
             threshold={:.2}dB:\
             ratio={:.2}:\
             attack={:.1}:\
             release={:.1}:\
             makeup={:.2}:\
             knee={:.2}",
            c.threshold_db, c.ratio, c.attack_ms, c.release_ms, c.makeup_db, c.knee_db,
        ));
    }

    // 4. True peak limiter — clamps peaks before loudnorm
    if preset.limiter.enabled {
        // Convert dBTP → linear level
        let level = 10f32.powf(preset.target.true_peak / 20.0);
        let l = &preset.limiter;
        filters.push(format!(
            "alimiter=\
             level_in=1:\
             level_out={level:.6}:\
             limit={level:.6}:\
             attack={:.1}:\
             release={:.1}:\
             asc=1",
            l.attack_ms, l.release_ms,
        ));
    }

    // 5. Two-pass loudnorm (linear mode — uses pass-1 measurements)
    let t = &preset.target;
    filters.push(format!(
        "loudnorm=\
         I={:.1}:\
         TP={:.1}:\
         LRA={:.1}:\
         measured_I={:.2}:\
         measured_LRA={:.2}:\
         measured_TP={:.2}:\
         measured_thresh={:.2}:\
         offset={:.2}:\
         linear=true:\
         print_format=summary",
        t.lufs, t.true_peak, t.lra,
        measured.i, measured.lra, measured.tp, measured.thresh, measured.offset,
    ));

    filters.join(",")
}

/// Map preset output config → ffmpeg codec name + extra args
fn codec_args(preset: &Preset) -> (&'static str, Vec<String>) {
    match preset.output.format.as_str() {
        "flac" => ("flac",        vec!["-compression_level".into(), "8".into()]),
        "mp3"  => ("libmp3lame",  vec!["-q:a".into(), "0".into()]),
        "aac"  => ("aac",         vec!["-b:a".into(), "320k".into()]),
        _      => {
            // Default: PCM WAV at chosen bit depth
            let codec = match preset.output.bit_depth {
                16 => "pcm_s16le",
                32 => "pcm_s32le",
                _  => "pcm_s24le",  // 24-bit default
            };
            (codec, vec![])
        }
    }
}

// ── Master run ────────────────────────────────────────────────────────────────

pub struct ProcessResult {
    pub filter_chain: String,
}

pub fn run_process(
    ffmpeg:   &Ffmpeg,
    input:    &Path,
    output:   &Path,
    preset:   &Preset,
    measured: &Measured,
    verbose:  bool,
) -> Result<ProcessResult> {
    let filter_chain = build_filter_chain(preset, measured);
    let (codec, extra) = codec_args(preset);

    let sample_rate = preset.output.sample_rate.to_string();
    let input_str   = input.to_str().unwrap_or("");
    let output_str  = output.to_str().unwrap_or("");

    let mut args: Vec<&str> = vec![
        "-hide_banner",
        "-y",               // overwrite output if exists
        "-i", input_str,
        "-af", &filter_chain,
        "-acodec", codec,
        "-ar", &sample_rate,
    ];

    // Borrow from owned strings so they live long enough
    let extra_refs: Vec<&str> = extra.iter().map(|s| s.as_str()).collect();
    for s in &extra_refs {
        args.push(s);
    }

    args.push(output_str);

    ffmpeg.run_verbose(&args, verbose)?;

    Ok(ProcessResult { filter_chain })
}
