use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ── Serde default free functions ──────────────────────────────────────────────

fn d_highpass()    -> f32    { 20.0         }
fn d_lowpass()     -> f32    { 0.0          }
fn d_comp_on()     -> bool   { false        }
fn d_comp_thr()    -> f32    { -18.0        }
fn d_comp_ratio()  -> f32    { 2.0          }
fn d_comp_atk()    -> f32    { 20.0         }
fn d_comp_rel()    -> f32    { 250.0        }
fn d_comp_make()   -> f32    { 0.0          }
fn d_comp_knee()   -> f32    { 2.0          }
fn d_lim_on()      -> bool   { true         }
fn d_lim_atk()     -> f32    { 5.0          }
fn d_lim_rel()     -> f32    { 50.0         }
fn d_fmt()         -> String { "wav".into() }
fn d_bits()        -> u32    { 24           }
fn d_sr()          -> u32    { 44100        }
fn d_viz_on()      -> bool   { true         }
fn d_viz_spec()    -> bool   { true         }
fn d_viz_wave()    -> bool   { true         }
fn d_viz_w()       -> u32    { 1920         }
fn d_viz_sh()      -> u32    { 512          }
fn d_viz_wh()      -> u32    { 200          }

// ── Structs ───────────────────────────────────────────────────────────────────

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

// [meta] and [target] are intentionally required — they are the identity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub name:        String,
    pub description: String,
    #[serde(default)] pub author: String,
    #[serde(default)] pub notes:  String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    pub lufs:      f32,
    pub true_peak: f32,
    pub lra:       f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filters {
    #[serde(default = "d_highpass")] pub highpass_hz: f32,
    #[serde(default = "d_lowpass")]  pub lowpass_hz:  f32,
}

impl Default for Filters {
    fn default() -> Self { Self { highpass_hz: d_highpass(), lowpass_hz: d_lowpass() } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Compressor {
    #[serde(default = "d_comp_on")]    pub enabled:      bool,
    #[serde(default = "d_comp_thr")]   pub threshold_db: f32,
    #[serde(default = "d_comp_ratio")] pub ratio:        f32,
    #[serde(default = "d_comp_atk")]   pub attack_ms:    f32,
    #[serde(default = "d_comp_rel")]   pub release_ms:   f32,
    #[serde(default = "d_comp_make")]  pub makeup_db:    f32,
    #[serde(default = "d_comp_knee")]  pub knee_db:      f32,
}

impl Default for Compressor {
    fn default() -> Self {
        Self {
            enabled:      d_comp_on(),
            threshold_db: d_comp_thr(),
            ratio:        d_comp_ratio(),
            attack_ms:    d_comp_atk(),
            release_ms:   d_comp_rel(),
            makeup_db:    d_comp_make(),
            knee_db:      d_comp_knee(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Limiter {
    #[serde(default = "d_lim_on")]  pub enabled:    bool,
    #[serde(default = "d_lim_atk")] pub attack_ms:  f32,
    #[serde(default = "d_lim_rel")] pub release_ms: f32,
}

impl Default for Limiter {
    fn default() -> Self {
        Self { enabled: d_lim_on(), attack_ms: d_lim_atk(), release_ms: d_lim_rel() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    #[serde(default = "d_fmt")]  pub format:      String,
    #[serde(default = "d_bits")] pub bit_depth:   u32,
    #[serde(default = "d_sr")]   pub sample_rate: u32,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self { format: d_fmt(), bit_depth: d_bits(), sample_rate: d_sr() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationConfig {
    #[serde(default = "d_viz_on")]   pub enabled:            bool,
    #[serde(default = "d_viz_spec")] pub spectrogram:        bool,
    #[serde(default = "d_viz_wave")] pub waveform:           bool,
    #[serde(default = "d_viz_w")]    pub width:              u32,
    #[serde(default = "d_viz_sh")]   pub spectrogram_height: u32,
    #[serde(default = "d_viz_wh")]   pub waveform_height:    u32,
}

impl Default for VisualizationConfig {
    fn default() -> Self {
        Self {
            enabled:            d_viz_on(),
            spectrogram:        d_viz_spec(),
            waveform:           d_viz_wave(),
            width:              d_viz_w(),
            spectrogram_height: d_viz_sh(),
            waveform_height:    d_viz_wh(),
        }
    }
}

impl Default for Preset {
    fn default() -> Self {
        Self {
            meta: Meta {
                name:        "default".into(),
                description: "General purpose — experimental electronic / abstract".into(),
                author:      String::new(),
                notes:       String::new(),
            },
            target:        Target { lufs: -16.0, true_peak: -1.0, lra: 11.0 },
            filters:       Filters::default(),
            compressor:    Compressor::default(),
            limiter:       Limiter::default(),
            output:        OutputConfig::default(),
            visualization: VisualizationConfig::default(),
        }
    }
}

// ── Preset resolution ─────────────────────────────────────────────────────────

pub fn preset_search_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![];
    dirs.push(PathBuf::from("presets"));
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("presets"));
        }
    }
    if let Some(cfg) = dirs::config_dir() {
        dirs.push(cfg.join("mastacraf").join("presets"));
    }
    dirs
}

pub fn load_preset(name: &str) -> Result<Preset> {
    let filename = format!("{name}.toml");

    for dir in preset_search_dirs() {
        let path = dir.join(&filename);
        if !path.exists() { continue; }

        let src = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        // Parse to a raw TOML table first so we can inject missing sections.
        // The toml crate will not trigger serde field-level defaults for a
        // struct field whose entire TOML section is absent — it only applies
        // defaults for keys missing *within* a present section. Injecting
        // empty tables for absent sections lets the field-level defaults fire.
        let mut table: toml::Table = toml::from_str(&src)
            .with_context(|| format!("Failed to parse {}", path.display()))?;

        for section in &["filters", "compressor", "limiter", "output", "visualization"] {
            table
                .entry(section.to_string())
                .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        }

        let preset: Preset = table
            .try_into()
            .with_context(|| format!("Failed to deserialize {}", path.display()))?;

        return Ok(preset);
    }

    if name == "default" {
        return Ok(Preset::default());
    }

    bail!(
        "Preset '{name}' not found.\n\
         Searched: {}\n\
         Run `mastacraf presets` to list available presets.",
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
        if !dir.exists() { continue; }
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
