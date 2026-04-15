use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ── Top-level preset ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub meta:          Meta,
    pub target:        Target,
    pub filters:       Filters,
    pub compressor:    Compressor,
    pub limiter:       Limiter,
    pub output:        OutputConfig,
    pub visualization: VisualizationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub name:        String,
    pub description: String,
    #[serde(default)]
    pub author:      String,
    #[serde(default)]
    pub notes:       String,
}

/// Loudness targets — the heart of a mastering preset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    /// Integrated loudness in LUFS (negative value, e.g. -16.0)
    pub lufs:       f32,
    /// True peak ceiling in dBTP (e.g. -1.0)
    pub true_peak:  f32,
    /// Loudness range in LU — higher = more dynamics preserved
    pub lra:        f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filters {
    /// High-pass cutoff in Hz (0 = disabled). Removes subsonic rumble / DC.
    pub highpass_hz: f32,
    /// Low-pass cutoff in Hz (0 = disabled). Rarely needed.
    pub lowpass_hz:  f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Compressor {
    pub enabled:      bool,
    pub threshold_db: f32,
    pub ratio:        f32,
    pub attack_ms:    f32,
    pub release_ms:   f32,
    pub makeup_db:    f32,
    pub knee_db:      f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Limiter {
    pub enabled:    bool,
    pub attack_ms:  f32,
    pub release_ms: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// "wav", "flac", "mp3", "aac"
    pub format:      String,
    /// Bit depth for WAV/FLAC (16, 24, 32)
    pub bit_depth:   u32,
    /// Output sample rate in Hz
    pub sample_rate: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationConfig {
    pub enabled:            bool,
    pub spectrogram:        bool,
    pub waveform:           bool,
    pub width:              u32,
    pub spectrogram_height: u32,
    pub waveform_height:    u32,
}

// ── Defaults — used when no preset file exists for "default" ─────────────────

impl Default for Preset {
    fn default() -> Self {
        Self {
            meta: Meta {
                name:        "default".into(),
                description: "General purpose — experimental electronic / abstract".into(),
                author:      String::new(),
                notes:       String::new(),
            },
            target: Target {
                lufs:      -16.0,
                true_peak: -1.0,
                lra:       11.0,
            },
            filters: Filters {
                highpass_hz: 20.0,
                lowpass_hz:  0.0,
            },
            compressor: Compressor {
                enabled:      false,
                threshold_db: -18.0,
                ratio:        2.0,
                attack_ms:    20.0,
                release_ms:   250.0,
                makeup_db:    0.0,
                knee_db:      2.0,
            },
            limiter: Limiter {
                enabled:    true,
                attack_ms:  5.0,
                release_ms: 50.0,
            },
            output: OutputConfig {
                format:      "wav".into(),
                bit_depth:   24,
                sample_rate: 44100,
            },
            visualization: VisualizationConfig {
                enabled:            true,
                spectrogram:        true,
                waveform:           true,
                width:              1920,
                spectrogram_height: 512,
                waveform_height:    200,
            },
        }
    }
}

// ── Preset resolution ─────────────────────────────────────────────────────────

/// Ordered list of directories to search for preset .toml files
pub fn preset_search_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![];

    // 1. ./presets/ relative to CWD  (primary for per-project presets)
    dirs.push(PathBuf::from("presets"));

    // 2. Executable-adjacent presets/ (for bundled / installed layout)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("presets"));
        }
    }

    // 3. User config dir  (~/.config/mastercraft/presets on Linux/macOS)
    if let Some(config_dir) = dirs::config_dir() {
        dirs.push(config_dir.join("mastercraft").join("presets"));
    }

    dirs
}

pub fn load_preset(name: &str) -> Result<Preset> {
    let filename = format!("{name}.toml");

    for dir in preset_search_dirs() {
        let path = dir.join(&filename);
        if path.exists() {
            let src = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            let preset: Preset = toml::from_str(&src)
                .with_context(|| format!("Failed to parse {}", path.display()))?;
            return Ok(preset);
        }
    }

    if name == "default" {
        return Ok(Preset::default());
    }

    bail!(
        "Preset '{name}' not found.\n\
         Searched: {}\n\
         Run `mastercraft presets` to list available presets.",
        preset_search_dirs()
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

pub fn list_presets() -> Result<Vec<(String, PathBuf)>> {
    let mut out: Vec<(String, PathBuf)> = vec![];

    for dir in preset_search_dirs() {
        if !dir.exists() {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "toml") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if !out.iter().any(|(n, _)| n == stem) {
                            out.push((stem.to_string(), path));
                        }
                    }
                }
            }
        }
    }

    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}
