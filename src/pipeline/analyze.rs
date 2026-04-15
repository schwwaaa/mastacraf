use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::ffmpeg::{Ffmpeg, NULL_DEV};

// ── Analysis result ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioAnalysis {
    pub integrated_lufs:  f32,
    pub true_peak_dbtp:   f32,
    pub loudness_range_lu: f32,
    pub threshold_lufs:   f32,
    pub duration_secs:    f32,
    pub sample_rate:      u32,
    pub channels:         u32,
    pub bit_depth:        Option<u32>,
    pub codec:            String,
}

impl AudioAnalysis {
    pub fn duration_display(&self) -> String {
        let s = self.duration_secs as u32;
        format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
    }
}

// ── Raw serde target for loudnorm JSON ───────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct LoudnormJson {
    pub input_i:              String,
    pub input_tp:             String,
    pub input_lra:            String,
    pub input_thresh:         String,
    #[serde(default)]
    pub output_i:             String,
    #[serde(default)]
    pub normalization_type:   String,
    #[serde(default)]
    pub target_offset:        String,
}

/// Parsed measurements ready for pass-2 loudnorm filter
#[derive(Debug, Clone)]
pub struct Measured {
    pub i:      f32,
    pub tp:     f32,
    pub lra:    f32,
    pub thresh: f32,
    pub offset: f32,
}

impl From<&LoudnormJson> for Measured {
    fn from(j: &LoudnormJson) -> Self {
        Measured {
            i:      j.input_i.parse().unwrap_or(-23.0),
            tp:     j.input_tp.parse().unwrap_or(-3.0),
            lra:    j.input_lra.parse().unwrap_or(7.0),
            thresh: j.input_thresh.parse().unwrap_or(-33.0),
            offset: j.target_offset.parse().unwrap_or(0.0),
        }
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Run full pre-analysis: file probe + loudnorm pass 1
pub fn analyze(ffmpeg: &Ffmpeg, input: &Path, verbose: bool) -> Result<(AudioAnalysis, Measured)> {
    let probe = probe_file(ffmpeg, input, verbose)?;
    let (json, measured) = loudnorm_pass1(ffmpeg, input, -16.0, -1.0, 11.0, verbose)?;

    let analysis = AudioAnalysis {
        integrated_lufs:   measured.i,
        true_peak_dbtp:    measured.tp,
        loudness_range_lu: measured.lra,
        threshold_lufs:    measured.thresh,
        duration_secs:     probe.duration,
        sample_rate:       probe.sample_rate,
        channels:          probe.channels,
        bit_depth:         probe.bit_depth,
        codec:             probe.codec,
    };

    let _ = json; // available if needed downstream
    Ok((analysis, measured))
}

/// Run loudnorm pass 1 with custom targets.
/// Returns raw JSON struct and the parsed Measured values.
pub fn loudnorm_pass1(
    ffmpeg:      &Ffmpeg,
    input:       &Path,
    target_lufs: f32,
    target_tp:   f32,
    target_lra:  f32,
    verbose:     bool,
) -> Result<(LoudnormJson, Measured)> {
    let input_str = input.to_str().unwrap_or("");
    let filter = format!(
        "loudnorm=I={target_lufs}:TP={target_tp}:LRA={target_lra}:print_format=json"
    );

    let out = ffmpeg.run_verbose(
        &["-hide_banner", "-i", input_str, "-af", &filter, "-f", "null", NULL_DEV],
        verbose,
    )?;

    let json = parse_loudnorm_json(&out.stderr)?;
    let measured = Measured::from(&json);
    Ok((json, measured))
}

// ── ffprobe-style data from ffmpeg stderr ────────────────────────────────────

struct ProbeData {
    duration:    f32,
    sample_rate: u32,
    channels:    u32,
    bit_depth:   Option<u32>,
    codec:       String,
}

fn probe_file(ffmpeg: &Ffmpeg, input: &Path, verbose: bool) -> Result<ProbeData> {
    let input_str = input.to_str().unwrap_or("");
    // Feed to ffmpeg -f null — it always prints file info to stderr
    let out = ffmpeg.run_verbose(
        &["-hide_banner", "-i", input_str, "-f", "null", NULL_DEV],
        verbose,
    );

    // ffmpeg exits 1 when -f null has no video stream — that's fine for audio
    let stderr = match out {
        Ok(o)  => o.stderr,
        Err(_) => {
            // Re-run without checking exit status
            ffmpeg.run(&["-hide_banner", "-i", input_str, "-f", "null", NULL_DEV])?.stderr
        }
    };

    Ok(ProbeData {
        duration:    parse_duration(&stderr).unwrap_or(0.0),
        sample_rate: parse_sample_rate(&stderr).unwrap_or(44100),
        channels:    parse_channels(&stderr).unwrap_or(2),
        bit_depth:   parse_bit_depth(&stderr),
        codec:       parse_codec(&stderr).unwrap_or_else(|| "unknown".into()),
    })
}

// ── Parsing helpers ───────────────────────────────────────────────────────────

fn parse_loudnorm_json(stderr: &str) -> Result<LoudnormJson> {
    // The JSON block is typically the last '{…}' in stderr
    let start = stderr.rfind('{').context("No JSON found in loudnorm output")?;
    let end   = stderr.rfind('}').context("No closing brace in loudnorm output")?;
    if end < start {
        bail!("Malformed JSON in loudnorm output");
    }
    let json = &stderr[start..=end];
    serde_json::from_str(json).context("Failed to parse loudnorm JSON")
}

fn parse_duration(stderr: &str) -> Option<f32> {
    for line in stderr.lines() {
        if line.contains("Duration:") {
            let after = line.splitn(2, "Duration:").nth(1)?;
            let time  = after.trim().splitn(2, ',').next()?.trim();
            let parts: Vec<&str> = time.splitn(3, ':').collect();
            if parts.len() == 3 {
                let h: f32 = parts[0].trim().parse().ok()?;
                let m: f32 = parts[1].trim().parse().ok()?;
                let s: f32 = parts[2].trim().parse().ok()?;
                return Some(h * 3600.0 + m * 60.0 + s);
            }
        }
    }
    None
}

fn parse_sample_rate(stderr: &str) -> Option<u32> {
    for line in stderr.lines() {
        if line.contains("Audio:") {
            for part in line.split(',') {
                let part = part.trim();
                if part.ends_with("Hz") {
                    if let Ok(sr) = part.replace(" Hz", "").trim().parse::<u32>() {
                        return Some(sr);
                    }
                }
            }
        }
    }
    None
}

fn parse_channels(stderr: &str) -> Option<u32> {
    for line in stderr.lines() {
        if line.contains("Audio:") {
            for part in line.split(',') {
                let p = part.trim();
                if p == "stereo"                     { return Some(2); }
                if p == "mono"                       { return Some(1); }
                if p.contains("5.1")                 { return Some(6); }
                if p.contains("7.1")                 { return Some(8); }
                if p.contains("quad")                { return Some(4); }
            }
        }
    }
    None
}

fn parse_bit_depth(stderr: &str) -> Option<u32> {
    for line in stderr.lines() {
        if line.contains("Audio:") {
            for part in line.split(',') {
                let p = part.trim();
                // pcm_s24le / pcm_s16le / fltp / etc.
                // Also appears as a format hint: "s24", "s16", "flt"
                for token in p.split_whitespace() {
                    let t = token.trim_start_matches("pcm_s")
                                 .trim_start_matches("pcm_u")
                                 .trim_end_matches("le")
                                 .trim_end_matches("be");
                    if let Ok(bd) = t.parse::<u32>() {
                        if matches!(bd, 8 | 16 | 24 | 32) {
                            return Some(bd);
                        }
                    }
                }
            }
        }
    }
    None
}

fn parse_codec(stderr: &str) -> Option<String> {
    for line in stderr.lines() {
        if line.contains("Audio:") {
            if let Some(after) = line.splitn(2, "Audio:").nth(1) {
                let codec = after.split(',').next()?.trim().to_string();
                if !codec.is_empty() {
                    return Some(codec);
                }
            }
        }
    }
    None
}
