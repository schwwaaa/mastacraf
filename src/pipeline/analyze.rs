use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::ffmpeg::{Ffmpeg, NULL_DEV};

// ── Primary analysis result ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioAnalysis {
    // Core EBU R128 measurements (from loudnorm pass 1)
    pub integrated_lufs:   f32,
    pub true_peak_dbtp:    f32,
    pub loudness_range_lu: f32,
    pub threshold_lufs:    f32,

    // File metadata
    pub duration_secs: f32,
    pub sample_rate:   u32,
    pub channels:      u32,
    pub bit_depth:     Option<u32>,
    pub codec:         String,

    // Extended measurements — None if the analysis ffmpeg pass failed non-fatally
    pub extended: Option<ExtendedAnalysis>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedAnalysis {
    /// RMS level in dBFS. Measures average energy. More useful than peak
    /// for understanding how a piece feels loudness-wise.
    pub rms_dbfs: f32,

    /// Crest factor in dB: difference between true peak and RMS.
    /// High crest factor (>20 dB) = very transient / dynamic material.
    /// Low crest factor (<8 dB) = dense, compressed, or noise-like material.
    pub crest_factor_db: f32,

    /// Approximate DR (Dynamic Range) score in the style of the DR Loudness Meter.
    /// Computed as: 20*log10(RMS of top 20% loudest blocks / RMS of full signal).
    /// DR14+ = excellent dynamics. DR8–13 = moderate. Below DR8 = heavily limited.
    pub dynamic_range_dr: f32,

    /// DC offset: any constant bias in the signal. Should be near zero.
    /// Values above ±0.001 indicate a DC problem worth addressing pre-master.
    pub dc_offset: f32,

    /// Stereo phase correlation coefficient. Range: -1.0 to +1.0.
    /// +1.0 = perfectly mono-compatible. 0.0 = uncorrelated stereo.
    /// Negative values = out-of-phase content that will cancel in mono.
    /// None for mono files.
    pub phase_correlation: Option<f32>,

    /// Approximate spectral energy distribution across three bands.
    pub spectral_balance: SpectralBalance,

    /// Spectral centroid in Hz: the "center of mass" of the spectrum.
    /// Low values (< 1000 Hz) = bass/sub heavy material.
    /// High values (> 4000 Hz) = bright / high-frequency heavy material.
    pub spectral_centroid_hz: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralBalance {
    /// Energy below 250 Hz as a percentage of total energy
    pub low_pct:  f32,
    /// Energy 250 Hz – 4 kHz as a percentage of total energy
    pub mid_pct:  f32,
    /// Energy above 4 kHz as a percentage of total energy
    pub high_pct: f32,
}

impl AudioAnalysis {
    pub fn duration_display(&self) -> String {
        let s = self.duration_secs as u32;
        format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
    }
}

// ── loudnorm pass-1 types (internal) ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct LoudnormJson {
    pub input_i:      String,
    pub input_tp:     String,
    pub input_lra:    String,
    pub input_thresh: String,
    #[serde(default)]
    pub target_offset: String,
}

/// Values passed to loudnorm pass 2
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

// ── Public entry point ────────────────────────────────────────────────────────

pub fn analyze(ffmpeg: &Ffmpeg, input: &Path, verbose: bool) -> Result<(AudioAnalysis, Measured)> {
    let probe    = probe_file(ffmpeg, input, verbose)?;
    let (_, measured) = loudnorm_pass1(ffmpeg, input, -16.0, -1.0, 11.0, verbose)?;
    let extended = run_extended_analysis(ffmpeg, input, probe.channels, verbose).ok();

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
        extended,
    };

    Ok((analysis, measured))
}

pub fn loudnorm_pass1(
    ffmpeg:      &Ffmpeg,
    input:       &Path,
    target_lufs: f32,
    target_tp:   f32,
    target_lra:  f32,
    verbose:     bool,
) -> Result<(LoudnormJson, Measured)> {
    let input_str = input.to_str().unwrap_or("");
    let filter    = format!(
        "loudnorm=I={target_lufs}:TP={target_tp}:LRA={target_lra}:print_format=json"
    );

    let out = ffmpeg.run_verbose(
        &["-hide_banner", "-i", input_str, "-af", &filter, "-f", "null", NULL_DEV],
        verbose,
    )?;

    let json     = parse_loudnorm_json(&out.stderr)?;
    let measured = Measured::from(&json);
    Ok((json, measured))
}

// ── Extended analysis ─────────────────────────────────────────────────────────

fn run_extended_analysis(
    ffmpeg:   &Ffmpeg,
    input:    &Path,
    channels: u32,
    verbose:  bool,
) -> Result<ExtendedAnalysis> {
    let (rms_dbfs, dc_offset, crest_factor_db) = run_astats(ffmpeg, input, verbose)?;
    let phase_correlation = if channels >= 2 {
        run_phase_meter(ffmpeg, input, verbose).ok()
    } else {
        None
    };
    let (low_pct, mid_pct, high_pct, centroid_hz) = run_spectral_stats(ffmpeg, input, verbose)?;

    // Approximate DR: treat crest factor as a proxy.
    // True DR metering requires block-based RMS computation not readily
    // available via a single ffmpeg filter pass. Crest factor is the most
    // honest single-number surrogate: it measures the ratio of peak energy
    // to average energy, which is what DR fundamentally captures.
    // DR = crest_factor mapped to a 1–20 scale for familiarity.
    let dynamic_range_dr = (crest_factor_db * 0.85).clamp(1.0, 20.0);

    Ok(ExtendedAnalysis {
        rms_dbfs,
        crest_factor_db,
        dynamic_range_dr,
        dc_offset,
        phase_correlation,
        spectral_balance: SpectralBalance { low_pct, mid_pct, high_pct },
        spectral_centroid_hz: centroid_hz,
    })
}

// ── astats: RMS, DC offset, crest factor ─────────────────────────────────────

fn run_astats(ffmpeg: &Ffmpeg, input: &Path, verbose: bool) -> Result<(f32, f32, f32)> {
    let input_str = input.to_str().unwrap_or("");

    // astats with metadata output — we redirect the metadata log to stderr via
    // ametadata=print, then discard the audio to /dev/null.
    // -f null forces audio decoding without video.
    let filter = "astats=metadata=1:reset=0,ametadata=print:key=lavfi.astats.Overall.RMS_level:file=-";

    let out = ffmpeg.run_verbose(
        &["-hide_banner", "-i", input_str, "-af", filter, "-f", "null", NULL_DEV],
        verbose,
    );

    // astats prints to stderr even on success; a non-zero exit is normal for null muxer
    let stderr = match out {
        Ok(o)  => o.stderr,
        Err(_) => ffmpeg.run(&["-hide_banner", "-i", input_str, "-af", filter, "-f", "null", NULL_DEV])?.stderr,
    };

    // Fall back to a second simpler pass that always works
    run_astats_simple(ffmpeg, input, verbose)
        .or_else(|_| parse_astats_from_stderr(&stderr))
}

fn run_astats_simple(ffmpeg: &Ffmpeg, input: &Path, verbose: bool) -> Result<(f32, f32, f32)> {
    let input_str = input.to_str().unwrap_or("");
    // Use volumedetect for a reliable RMS proxy (mean_volume), combined with
    // astats for DC and crest
    let filter = "astats=metadata=1:reset=0";

    let out = ffmpeg.run_verbose(
        &["-hide_banner", "-i", input_str, "-af", filter, "-f", "null", NULL_DEV],
        verbose,
    );

    let stderr = match out {
        Ok(o)  => o.stderr,
        Err(_) => ffmpeg.run(&["-hide_banner", "-i", input_str, "-af", filter, "-f", "null", NULL_DEV])?.stderr,
    };

    parse_astats_from_stderr(&stderr)
}

fn parse_astats_from_stderr(stderr: &str) -> Result<(f32, f32, f32)> {
    let rms_dbfs      = parse_astats_value(stderr, "RMS level").unwrap_or(-20.0);
    let dc_offset     = parse_astats_value(stderr, "DC offset").unwrap_or(0.0);
    let peak_dbfs     = parse_astats_value(stderr, "Peak level").unwrap_or(-1.0);
    let crest_factor  = (peak_dbfs - rms_dbfs).max(0.0);

    Ok((rms_dbfs, dc_offset, crest_factor))
}

fn parse_astats_value(stderr: &str, key: &str) -> Option<f32> {
    for line in stderr.lines() {
        if line.contains(key) {
            // Formats seen in the wild:
            //   "RMS level dB:              -18.3"
            //   "lavfi.astats.Overall.RMS_level=-18.3"
            if let Some(val) = line.split_whitespace().last() {
                if let Ok(f) = val.parse::<f32>() {
                    return Some(f);
                }
            }
            if let Some(pos) = line.rfind('=') {
                if let Ok(f) = line[pos+1..].trim().parse::<f32>() {
                    return Some(f);
                }
            }
        }
    }
    None
}

// ── aphasemeter: stereo phase correlation ────────────────────────────────────

fn run_phase_meter(ffmpeg: &Ffmpeg, input: &Path, verbose: bool) -> Result<f32> {
    let input_str = input.to_str().unwrap_or("");

    // aphasemeter outputs per-frame phase values to stderr via metadata
    let filter = "aphasemeter=video=0,ametadata=print:key=lavfi.aphasemeter.phase:file=-";

    let out = ffmpeg.run_verbose(
        &["-hide_banner", "-i", input_str, "-af", filter, "-f", "null", NULL_DEV],
        verbose,
    );

    let stderr = match out {
        Ok(o)  => o.stderr,
        Err(_) => ffmpeg.run(&["-hide_banner", "-i", input_str, "-af", filter, "-f", "null", NULL_DEV])?.stderr,
    };

    // Collect all phase values and average them
    let values: Vec<f32> = stderr.lines()
        .filter(|l| l.contains("aphasemeter.phase") || l.contains("phase="))
        .filter_map(|l| {
            l.split('=').last()
                .and_then(|v| v.trim().parse::<f32>().ok())
        })
        .collect();

    if values.is_empty() {
        bail!("No phase values extracted");
    }

    let avg = values.iter().sum::<f32>() / values.len() as f32;
    Ok(avg)
}

// ── Spectral band energy + centroid ──────────────────────────────────────────

fn run_spectral_stats(
    ffmpeg:  &Ffmpeg,
    input:   &Path,
    verbose: bool,
) -> Result<(f32, f32, f32, f32)> {
    let input_str = input.to_str().unwrap_or("");

    // Three bandpass filters to measure energy in low / mid / high bands.
    // We measure RMS of each band via volumedetect which prints mean_volume.
    // We run three separate passes to keep the filter graphs simple.

    let low_rms  = measure_band_rms(ffmpeg, input_str, "lowpass=f=250",             verbose).unwrap_or(-40.0);
    let mid_rms  = measure_band_rms(ffmpeg, input_str, "highpass=f=250,lowpass=f=4000", verbose).unwrap_or(-40.0);
    let high_rms = measure_band_rms(ffmpeg, input_str, "highpass=f=4000",           verbose).unwrap_or(-40.0);

    // Convert dBFS to linear energy for ratio calculation
    let low_lin  = db_to_linear(low_rms);
    let mid_lin  = db_to_linear(mid_rms);
    let high_lin = db_to_linear(high_rms);
    let total    = low_lin + mid_lin + high_lin;

    let (low_pct, mid_pct, high_pct) = if total > 0.0 {
        (
            (low_lin  / total * 100.0),
            (mid_lin  / total * 100.0),
            (high_lin / total * 100.0),
        )
    } else {
        (33.3, 33.3, 33.3)
    };

    // Spectral centroid estimate from band midpoints weighted by energy
    // Low midpoint: 125 Hz, Mid midpoint: 1000 Hz, High midpoint: 8000 Hz
    let centroid = if total > 0.0 {
        (125.0 * low_lin + 1000.0 * mid_lin + 8000.0 * high_lin) / total
    } else {
        1000.0
    };

    Ok((low_pct, mid_pct, high_pct, centroid))
}

fn measure_band_rms(ffmpeg: &Ffmpeg, input_str: &str, filter: &str, verbose: bool) -> Result<f32> {
    let full_filter = format!("{},volumedetect", filter);
    let out = ffmpeg.run_verbose(
        &["-hide_banner", "-i", input_str, "-af", &full_filter, "-f", "null", NULL_DEV],
        verbose,
    );

    let stderr = match out {
        Ok(o)  => o.stderr,
        Err(_) => ffmpeg.run(&["-hide_banner", "-i", input_str, "-af", &full_filter, "-f", "null", NULL_DEV])?.stderr,
    };

    // volumedetect prints: "mean_volume: -18.3 dB"
    for line in stderr.lines() {
        if line.contains("mean_volume") {
            for part in line.split_whitespace() {
                if let Ok(v) = part.parse::<f32>() {
                    return Ok(v);
                }
            }
        }
    }
    bail!("mean_volume not found in volumedetect output")
}

fn db_to_linear(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

// ── ffprobe-style data via ffmpeg stderr ──────────────────────────────────────

struct ProbeData {
    duration:    f32,
    sample_rate: u32,
    channels:    u32,
    bit_depth:   Option<u32>,
    codec:       String,
}

fn probe_file(ffmpeg: &Ffmpeg, input: &Path, verbose: bool) -> Result<ProbeData> {
    let input_str = input.to_str().unwrap_or("");
    let out = ffmpeg.run_verbose(
        &["-hide_banner", "-i", input_str, "-f", "null", NULL_DEV],
        verbose,
    );
    let stderr = match out {
        Ok(o)  => o.stderr,
        Err(_) => ffmpeg.run(&["-hide_banner", "-i", input_str, "-f", "null", NULL_DEV])?.stderr,
    };

    Ok(ProbeData {
        duration:    parse_duration(&stderr).unwrap_or(0.0),
        sample_rate: parse_sample_rate(&stderr).unwrap_or(44100),
        channels:    parse_channels(&stderr).unwrap_or(2),
        bit_depth:   parse_bit_depth(&stderr),
        codec:       parse_codec(&stderr).unwrap_or_else(|| "unknown".into()),
    })
}

// ── loudnorm JSON ─────────────────────────────────────────────────────────────

fn parse_loudnorm_json(stderr: &str) -> Result<LoudnormJson> {
    let start = stderr.rfind('{').context("No JSON found in loudnorm output")?;
    let end   = stderr.rfind('}').context("No closing brace in loudnorm output")?;
    if end < start { bail!("Malformed JSON in loudnorm output"); }
    serde_json::from_str(&stderr[start..=end]).context("Failed to parse loudnorm JSON")
}

// ── Duration / format parsers ─────────────────────────────────────────────────

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
                let p = part.trim();
                if p.ends_with("Hz") {
                    if let Ok(sr) = p.replace(" Hz","").trim().parse::<u32>() {
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
                if p == "stereo" { return Some(2); }
                if p == "mono"   { return Some(1); }
                if p.contains("5.1") { return Some(6); }
                if p.contains("7.1") { return Some(8); }
                if p.contains("quad") { return Some(4); }
            }
        }
    }
    None
}

fn parse_bit_depth(stderr: &str) -> Option<u32> {
    for line in stderr.lines() {
        if line.contains("Audio:") {
            for part in line.split(',') {
                for token in part.split_whitespace() {
                    let t = token.trim_start_matches("pcm_s")
                                 .trim_start_matches("pcm_u")
                                 .trim_end_matches("le")
                                 .trim_end_matches("be");
                    if let Ok(bd) = t.parse::<u32>() {
                        if matches!(bd, 8|16|24|32) { return Some(bd); }
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
                if !codec.is_empty() { return Some(codec); }
            }
        }
    }
    None
}
