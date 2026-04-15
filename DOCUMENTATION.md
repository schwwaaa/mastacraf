# mastacraf documentation

**Version:** 0.1.0  
**License:** MIT

---

## Table of contents

1. [Overview](#overview)
2. [How it works](#how-it-works)
3. [Installation](#installation)
4. [Quick start](#quick-start)
5. [CLI reference](#cli-reference)
6. [Creating a preset](#creating-a-preset)
7. [Preset field reference](#preset-field-reference)
8. [Analysis — what the measurements mean](#analysis--what-the-measurements-mean)
9. [The filter chain](#the-filter-chain)
10. [Output folder structure](#output-folder-structure)
11. [Visualization output](#visualization-output)
12. [The mastering report](#the-mastering-report)
13. [Extending the pipeline](#extending-the-pipeline)
14. [AI integration](#ai-integration)
15. [Bundling (Tauri)](#bundling-tauri)
16. [Troubleshooting](#troubleshooting)

---

## Overview

mastacraf is a command-line audio mastering pipeline built in Rust, using FFmpeg as its sole processing core. It is designed for mastering self-produced, self-mixed experimental electronic music where no commercial loudness standard applies and the goal is a repeatable, documented, and tunable process you own entirely.

It does not make aesthetic decisions. It applies the processing chain you define in a preset file, measures the result, and writes a report documenting exactly what happened.

**What it does:**

- Measures integrated loudness (LUFS), true peak, loudness range, RMS level, crest factor, dynamic range, DC offset, phase correlation, and spectral balance of your input file
- Applies a configurable filter chain: high-pass, optional low-pass, optional compression, peak limiting, and two-pass EBU R128 loudness normalization
- Outputs all files for a given track into a dedicated subfolder named after the track stem
- Generates before/after spectrogram and waveform images
- Writes a JSON report containing all measurements, the exact FFmpeg filter string used, and a delta between input and output

**What it does not do:**

- Make EQ decisions
- Decide compression settings for you
- Automatically match loudness to a reference track
- Alter stereo field, phase, or any aspect of the mix outside the defined chain

---

## How it works

### The two-pass loudnorm process

The core of every master run is FFmpeg's `loudnorm` filter, run twice.

**Pass 1 (analysis):** The entire file is decoded and fed through loudnorm in measurement-only mode. No audio is written. The filter outputs a JSON block containing the file's measured integrated loudness (LUFS), true peak (dBTP), loudness range (LRA), and threshold.

**Pass 2 (processing):** The file is decoded again through the full filter chain. The loudnorm filter on this pass receives the measurements from pass 1 as parameters and applies a linear gain adjustment. Because it has the full-file measurements, it applies a precise, artifact-free gain change rather than estimating in real time.

Single-pass loudnorm estimates and will overshoot or undershoot depending on material. Two-pass always hits the target.

### Extended analysis passes

After pass 1, the tool runs three additional FFmpeg passes to collect the extended analysis data:

1. **`astats` pass:** Measures RMS level, peak level (for crest factor), and DC offset across the full file.
2. **`aphasemeter` pass (stereo only):** Measures per-frame stereo phase correlation and averages it across the file.
3. **Three band-energy passes:** Runs `volumedetect` on three bandpass-filtered versions of the signal (low/mid/high) to compute spectral balance and estimate the spectral centroid.

These passes run sequentially. On a typical track they add 3–6 seconds of total analysis time beyond pass 1.

### Filter chain execution

The filter chain is a comma-separated FFmpeg `-af` string. FFmpeg processes it as a linear graph. The chain is built in `src/pipeline/process.rs` by `build_filter_chain()`. Disabled stages are not included in the string and have zero overhead.

---

## Installation

### Prerequisites

- Rust toolchain: https://rustup.rs
- FFmpeg in your `$PATH` or placed alongside the compiled binary

```sh
# macOS
brew install ffmpeg

# Ubuntu / Debian
sudo apt install ffmpeg

# Arch
sudo pacman -S ffmpeg

# Windows
# Download from https://ffmpeg.org/download.html and add to PATH
```

### Build

```sh
git clone <repo>
cd mastacraf
cargo build --release
# binary: ./target/release/mastacraf
```

### Verify

```sh
mastacraf --version
mastacraf --help
```

---

## Quick start

```sh
# Analyze first — always do this before writing a preset
mastacraf analyze track.wav

# Master with default preset
mastacraf master track.wav

# Master with a named preset
mastacraf master track.wav --preset noise

# Test a preset without writing anything
mastacraf master track.wav --preset mypreset --dry-run
```

---

## CLI reference

### `mastacraf master`

```
mastacraf master <INPUT> [OPTIONS]
```

| Flag / Arg        | Default        | Description |
|-------------------|----------------|-------------|
| `<INPUT>`         | required       | Input audio file. Any FFmpeg-decodable format. |
| `-p`, `--preset`  | `default`      | Preset name. Searches `./presets/<n>.toml`. |
| `-o`, `--output`  | `./mastered/`  | Base output directory. A subfolder named after the input stem is created inside it. |
| `--suffix`        | `_master`      | Appended to input stem for the output filename. |
| `--lufs`          | from preset    | Override integrated loudness target. |
| `--true-peak`     | from preset    | Override true peak ceiling. |
| `--no-visualize`  | off            | Skip image generation. |
| `--dry-run`       | off            | Analyze and print measurements. Write nothing. |
| `-v`, `--verbose` | off            | Print every FFmpeg command as it runs. |

**Output folder for each run:**
```
<base>/<input_stem>/
  <stem><suffix>.<ext>
  <stem>_pre_spectrogram.png
  <stem>_pre_waveform.png
  <stem>_post_spectrogram.png
  <stem>_post_waveform.png
  <stem><suffix>_report.json
```

---

### `mastacraf analyze`

```
mastacraf analyze <INPUT> [OPTIONS]
```

Runs full analysis: loudnorm pass 1 + extended passes (RMS, crest, phase, spectral balance). Prints all measurements to stdout. No audio written.

| Flag / Arg        | Default | Description |
|-------------------|---------|-------------|
| `<INPUT>`         | required | Input file. |
| `--visualize`     | off      | Generate spectrogram and waveform. |
| `-o`, `--output`  | `./`     | Directory for visualization images. |
| `-v`, `--verbose` | off      | Print FFmpeg commands. |

---

### `mastacraf presets`

Lists all presets found in the search path. Prints name, LUFS, true peak, LRA, description.

---

### `mastacraf preset <n>`

Prints the full content of a named preset to stdout.

```sh
mastacraf preset film
mastacraf preset noise > mybase.toml
```

---

## Creating a preset

This is the core workflow. A preset is a TOML file that fully defines one mastering configuration. Every audio-affecting field maps directly to one FFmpeg filter parameter. There is no hidden processing.

### Step 1: Analyze the material

```sh
mastacraf analyze track.wav
```

Read every value in the output before touching a preset file. Specifically note:

- **Integrated loudness** — how far the loudnorm gain will push the signal. If measured is −22 LUFS and target is −14, loudnorm adds 8 dB, which also raises the true peak by 8 dB. The limiter needs to handle whatever the new true peak becomes.
- **Loudness range** — this is your floor for `target.lra`. If the measured LRA is 14 LU and you set `target.lra = 8`, loudnorm will compress 6 LU of dynamic range away with no warning. Set `target.lra` at or above the measured value.
- **True peak** — tells you how aggressively the limiter will work. If measured TP is −0.3 dBTP and target is −1.0, the limiter is catching almost every peak and may introduce artifacts on transient material.
- **Crest factor** — high (>18 dB) means very transient / dynamic material. The limiter attack should be slower (5–10ms) to preserve transient shape. Low (<8 dB) means dense material; fast attack (0.5–1ms) is fine.
- **Phase correlation** — if this is below 0.3 or negative, there is a stereo phase issue in the mix. The master will not fix this. Address it at the mix stage before mastering.
- **Spectral balance** — if 70%+ of energy is in the low band, the highpass setting is largely irrelevant to the sound but the loudnorm will be responding mostly to low-frequency energy. If the high band is dominant (noise, feedback material), the limiter may behave differently than expected.

### Step 2: Decide where the file is going

This determines `target.lufs` and `target.true_peak`.

| Destination          | lufs   | true_peak | Notes |
|----------------------|--------|-----------|-------|
| Streaming (general)  | -16.0  | -1.0      | Spotify −14, Apple −16; −16 avoids all platform normalization |
| Film / broadcast     | -23.0  | -2.0      | EBU R128 standard. Non-negotiable for delivery |
| Vinyl prep           | -12.0  | -0.3      | More headroom, cutting lathe will add its own limiting |
| DJ pool              | -9.0   | -0.5      | Clubs are loud; DJ software normalizes but loud files translate better |
| Personal / archive   | -18.0  | -1.0      | More headroom, dynamics preserved, no platform will touch it |
| Unknown / safe       | -16.0  | -1.0      | Safe default for anything |

### Step 3: Copy and edit a preset

```sh
cp presets/default.toml presets/mypreset.toml
```

Open `mypreset.toml`. Make these changes:

1. Set `[meta].name` to match the filename stem (`mypreset`)
2. Set `[meta].description` to describe what this preset is for
3. Set `[target].lufs` to your destination target
4. Set `[target].true_peak` to your destination target
5. Set `[target].lra` to the measured LRA value or higher
6. Decide on `[compressor].enabled` — see the compressor section in the field reference
7. Set `[limiter].attack_ms` based on the measured crest factor: high crest = slower attack, low crest = faster attack
8. Set `[output].sample_rate` to 48000 if going into a film pipeline, otherwise 44100

### Step 4: Test without writing

```sh
mastacraf master track.wav -p mypreset --dry-run
```

This runs pass 1 and prints pre-master measurements. You are not writing anything.

### Step 5: Run and check the report

```sh
mastacraf master track.wav -p mypreset
cat mastered/track/track_master_report.json
```

Check the `delta` section:

- `lra_change` should be near zero or slightly positive (dynamics preserved or expanded slightly). A large negative value (e.g. −4 LU) means `target.lra` is set too low. Raise it and re-run.
- `lufs_change` should reflect what you expected (measured − target).
- `crest_factor_change` should also be near zero. A large negative value means the limiter is hitting hard — consider raising `target.lufs` slightly or checking whether the true peak headroom is sufficient.

### Step 6: Listen and iterate

The numbers are a verification tool, not the goal. Listen to the output. If it sounds right, the preset is done. Keep it. Name it after what it was made for.

---

## Preset field reference

### `[target]`

#### `lufs`
Integrated loudness target in LUFS. The loudnorm filter adjusts the overall gain of the file to hit this value.  
**Range:** −32.0 to −9.0 | **Default:** −16.0

Lower values = quieter master with more headroom. The loudness target shifts the overall gain only — it does not affect relative dynamics within the track.

#### `true_peak`
Ceiling for intersample peaks in dBTP. Both the limiter and loudnorm enforce this value.  
**Range:** −3.0 to −0.1 | **Default:** −1.0

Intersample peaks occur when a DAC reconstructs the waveform between samples. A file where no sample exceeds 0 dBFS can still clip on playback. Never set this to 0.0.

#### `lra`
Loudness range target in Loudness Units.  
**Range:** 1.0 to 20.0 | **Default:** 11.0

**This is the most important value to get right.** The loudnorm filter attempts to fit the material's dynamic range into this target. If the material's natural LRA exceeds the target, loudnorm compresses dynamics to make it fit, silently. Set at or above the measured LRA from `mastacraf analyze`.

---

### `[filters]`

#### `highpass_hz`
Removes all audio below this frequency.  
**Range:** 0 (disabled) to 120.0 | **Default:** 20.0

Content below 20 Hz is inaudible but contributes to energy measured by loudnorm and occupies limiter headroom. For abstract/noise music where sub-bass is compositional, lower this to 12 Hz. Set to 0 to disable entirely.

#### `lowpass_hz`
Removes all audio above this frequency.  
**Range:** 0 (disabled) to 24000.0 | **Default:** 0.0

Disabled by default. Only relevant for specific deliverable format requirements.

---

### `[compressor]`

**Default: disabled.** The compressor processes the stereo bus simultaneously. It responds to the sum of all elements and changes the relationship between them. If you mixed it, you already made those decisions. Enable only with a specific reason.

#### `enabled`
`true` or `false`. Default: `false`.

#### `threshold_db`
Level above which compression engages. Everything below is untouched.  
**Range:** −60.0 to 0.0 | **Default:** −18.0

Lower threshold = compressor engages on more of the material. For mastering wide-range material, −18 to −12 dB engages on louder sections only.

#### `ratio`
Gain reduction above threshold. 2:1 means every 2 dB over the threshold becomes 1 dB.  
**Range:** 1.0 to 20.0 | **Default:** 2.0

For mastering: 1.5–2.0 is transparent-to-gentle. 4.0 is noticeable. Above 8.0 is limiting territory.

#### `attack_ms`
Response time when signal exceeds threshold.  
**Range:** 0.1 to 200.0 ms | **Default:** 20.0

Fast attack (1–5ms) catches transients. Slow attack (20–80ms) lets transients through before the compressor clamps. For material with intentionally designed transients, use slow attack.

#### `release_ms`
Recovery time after signal drops below threshold.  
**Range:** 10.0 to 2000.0 ms | **Default:** 250.0

Fast release can cause pumping on dense material. Slow release is more transparent but can hold gain reduction into subsequent quiet passages.

#### `makeup_db`
Gain added after compression.  
**Range:** 0.0 to 24.0 | **Default:** 0.0

The loudnorm stage after the compressor normalizes the final level regardless of this value. Makeup gain here effectively shifts the operating threshold, not the output level.

#### `knee_db`
Transition zone width around the threshold.  
**Range:** 0.0 to 12.0 | **Default:** 2.0

0 = hard knee (instant compression at threshold). 6 = soft knee (gradual onset). Soft knee sounds more natural on material without conventional dynamic structure.

---

### `[limiter]`

#### `enabled`
Default: `true`. Keep this on.

#### `attack_ms`
Response time when a peak is detected.  
**Range:** 0.1 to 20.0 ms | **Default:** 5.0

Use the measured crest factor to set this:
- Crest factor > 18 dB (very transient): use 5–10ms to preserve transient shape
- Crest factor 10–18 dB (moderate): use 2–5ms
- Crest factor < 10 dB (dense / noise): use 0.5–2ms, catch everything

#### `release_ms`
Recovery time after a peak.  
**Range:** 10.0 to 500.0 ms | **Default:** 50.0

Short (20–50ms): recovers fast, can cause pumping on dense material with many consecutive peaks. Long (100–300ms): smoother but can hold gain reduction into quieter passages.

---

### `[output]`

#### `format`
`wav` | `flac` | `mp3` | `aac` | **Default:** `wav`

For archival and delivery, use `wav` or `flac`. MP3/AAC only for platform-specific delivery when an uncompressed master already exists.

#### `bit_depth`
`16` | `24` | `32` | **Default:** `24`

24-bit is the mastering standard. 16-bit for CD replication only. 32-bit float for intermediates going into another DAW.

#### `sample_rate`
**Default:** `44100`

44100 Hz for music delivery. 48000 Hz for anything going into a film or video pipeline. Never upsample — setting this higher than the input creates a file with no additional information.

---

### `[visualization]`

#### `enabled`
Master switch. `true` or `false`. Can be overridden per-run with `--no-visualize`.

#### `spectrogram` / `waveform`
Individual toggles for each image type.

#### `width`
Horizontal resolution of images in pixels. **Default:** 1920. Increase to 3840+ for long pieces where time resolution matters.

#### `spectrogram_height` / `waveform_height`
Vertical resolution. **Defaults:** 512 / 200. Increase `spectrogram_height` to 768 or 1024 for content with dense spectral activity across a wide range.

---

## Analysis — what the measurements mean

### Core (EBU R128)

**Integrated loudness (LUFS)** — Average perceived loudness of the entire track, computed over all non-silent sections using ITU-R BS.1770. This is the number streaming platforms use for normalization. Use it to set `target.lufs`.

**True peak (dBTP)** — Maximum intersample peak, measured at 4× oversampling. Use it to set `target.true_peak`.

**Loudness range (LU)** — Statistical spread between quiet and loud sections (short-term loudness, 10th to 95th percentile). A classical piece may measure 20+ LU. A wall of noise may measure 2–3 LU. Set `target.lra` at or above this value.

**Threshold (LUFS)** — Loudness gate used during measurement. Sections below this level are excluded from the integrated loudness calculation.

---

### Extended analysis

**RMS level (dBFS)** — Root mean square of the signal: the average energy, not the peak. More representative than peak level of how loud something actually sounds. If RMS is −22 dBFS and true peak is −0.5 dBTP, the crest factor is 21.5 dB — very dynamic.

**Crest factor (dB)** — Difference between true peak and RMS level. Measures the ratio of transient peaks to average energy.

| Crest factor | Character |
|---|---|
| > 20 dB | Very transient / dynamic (acoustic, experimental, wide-range) |
| 14–20 dB | Normal range for mixed material |
| 8–14 dB | Dense, compressed, or sustained |
| < 8 dB | Heavily limited, noise-like, or deliberately saturated |

Use crest factor to set `limiter.attack_ms`. High crest = slower attack to preserve transient shape. Low crest = faster attack.

**Dynamic range (DR)** — Approximate DR score in the style of the Pleasurize Music Foundation DR Meter. Derived from the crest factor (a direct measurement of the same ratio DR formalizes). DR14+ is considered excellent dynamics. DR8–13 is moderate. Below DR8 indicates heavy limiting or compression already in the mix.

**DC offset** — A constant bias in the signal at 0 Hz. Ideally zero. Values above ±0.001 indicate a real DC problem. DC offset consumes headroom, can cause clicks at cut points, and can cause the highpass filter to produce a click at the beginning of the file. Address DC offset at the mix or recording stage. A highpass filter at 20 Hz will remove most DC offset in practice, but if the value is large (above ±0.005), investigate the source.

**Phase correlation** — Measures how correlated the left and right channels are, averaged across the file.

| Value | Meaning |
|---|---|
| +1.0 | Perfectly mono-compatible. Identical L and R. |
| +0.8 to +1.0 | Strong mono compatibility. Normal for centered material. |
| +0.3 to +0.8 | Normal stereo. Good mono compatibility. |
| 0.0 to +0.3 | Wide stereo or uncorrelated content. Check mono fold-down. |
| -0.2 to 0.0 | Approaching out-of-phase. Some cancellation in mono. |
| Below -0.2 | Significant phase problem. Material will partially cancel in mono. |

Phase correlation below 0.3 or negative is a mix problem, not a mastering problem. The master does not address it. Fix it at the mix stage.

**Spectral balance (%, three bands)** — Approximate percentage of total energy in each band:
- Low band: below 250 Hz
- Mid band: 250 Hz – 4 kHz
- High band: above 4 kHz

This is a rough diagnostic, not a precise metering tool. Use it to understand the general character of the material before choosing EQ adjustments. A typical balanced mix might show 35% low / 45% mid / 20% high. Noise and high-frequency-heavy material may show 5% / 20% / 75%. Sub-heavy material might show 70% / 25% / 5%.

**Spectral centroid (Hz)** — The "center of mass" of the spectrum, computed as an energy-weighted average of the three band midpoints. A rough single-number indicator of spectral brightness.

| Range | Character |
|---|---|
| < 400 Hz | Sub/bass-dominant |
| 400–1500 Hz | Low-mid heavy |
| 1500–3000 Hz | Balanced |
| 3000–5000 Hz | Bright / presence-heavy |
| > 5000 Hz | Very bright / high-frequency dominant |

---

## The filter chain

### Structure and order (fixed)

```
highpass → lowpass → compressor → limiter → loudnorm
```

Built in `src/pipeline/process.rs`, function `build_filter_chain()`. Disabled stages are omitted entirely.

### Example output (default preset)

```
highpass=f=20.0,alimiter=level_in=1:level_out=0.891251:limit=0.891251:attack=5.0:release=50.0:asc=1,loudnorm=I=-16.0:TP=-1.0:LRA=11.0:measured_I=-18.43:measured_LRA=14.2:measured_TP=-2.1:measured_thresh=-28.7:offset=0.0:linear=true:print_format=summary
```

### Viewing the chain used

```sh
mastacraf master track.wav -v           # prints it during the run
cat mastered/track/track_master_report.json | python3 -m json.tool | grep filter_chain
```

### True peak conversion

The limiter's `level_out` takes a linear amplitude value. The tool converts `target.true_peak` from dBTP automatically:

```
linear = 10 ^ (true_peak / 20)

-0.5 dBTP  → 0.944061
-1.0 dBTP  → 0.891251
-2.0 dBTP  → 0.794328
-3.0 dBTP  → 0.707946
```

---

## Output folder structure

Each master run creates a dedicated subfolder inside the base output directory, named after the input file stem:

```
mastered/
  track01/
    track01_master.wav

    track01_pre_spectrogram.png       individual pre-master spectrogram
    track01_pre_waveform.png          individual pre-master waveform
    track01_post_spectrogram.png      individual post-master spectrogram
    track01_post_waveform.png         individual post-master waveform

    track01_compare_spectrogram.png   pre (top) + post (bottom), amber separator
    track01_compare_waveform.png      pre (top) + post (bottom), amber separator
    track01_diff_spectrogram.png      contrast-amplified pixel difference
    track01_diff_waveform.png         contrast-amplified pixel difference

    track01_master_report.json
  track02/
    ...
```

To change the base directory:
```sh
mastacraf master track01.wav -o ./album_masters/
# → mastered files go to ./album_masters/track01/
```

The subfolder is always the input stem. The `--suffix` flag affects only the audio output filename, not the folder:
```sh
mastacraf master track01.wav --suffix _v2
# folder:     mastered/track01/
# audio file: mastered/track01/track01_v2.wav
```

---

## Visualization output

Six images are generated per master run: individual pre and post visuals, a stacked comparison, and a pixel-difference image. All are written into the track's output subfolder.

### Individual visuals (generated per pass)

**`_pre_spectrogram.png` / `_post_spectrogram.png`**

FFmpeg filter: `showspectrumpic=mode=combined:color=intensity:scale=log:legend=1`

- X axis: time
- Y axis: frequency, logarithmic scale — each octave gets equal vertical space
- Color: intensity — darker = louder at that frequency and time
- Legend: frequency axis labels on the right edge

**`_pre_waveform.png` / `_post_waveform.png`**

FFmpeg filter: `showwavespic=split_channels=1`

- X axis: time
- Y axis: amplitude (0 at center)
- L channel top, R channel below, distinct colors

---

### Stacked comparison (`_compare_spectrogram.png`, `_compare_waveform.png`)

Pre image on top, post image below, separated by a 3px amber dividing line. Both images are aligned on the same time axis, so any section of the track can be compared vertically by eye.

FFmpeg filter complex:
```
[pre][post] → pad pre bottom +3px (amber) → vstack
```

**Reading the stacked spectrogram:** Scan vertically at any time position. Frequency content, density, and brightness should be similar between top and bottom. A band that is brighter in the post (bottom) image means that frequency region gained energy. A band that is darker means it lost energy. Loudnorm adds only linear gain, so the overall brightness difference should be uniform — non-uniform differences indicate the compressor or limiter is shaping specific moments.

**Reading the stacked waveform:** The envelope shape should be visually similar. If the post waveform envelope has a noticeably different shape than the pre waveform, the LRA target may be set too low and loudnorm is compressing dynamics.

---

### Diff image (`_diff_spectrogram.png`, `_diff_waveform.png`)

Pixel-by-pixel absolute difference between pre and post, processed through an aggressive contrast amplification curve and a saturation boost. These images answer one question: **where did anything change, and how much?**

FFmpeg filter complex:
```
[pre][post] → blend=difference → curves (amplify contrast) → hue=s=6 (saturate)
```

**Interpreting the diff spectrogram:**

- **Black / near-black regions** — no change. The mastering pipeline made no audible difference to those frequencies at those moments.
- **Bright regions** — significant change. This could be the limiter attenuating a transient peak, the loudnorm gain change affecting a loud section, or the compressor engaging.
- **Hue in bright regions** — because the diff is computed from the original spectrogram pixels (which use hue to encode frequency energy direction), the color in changed regions carries spectral meaning. A yellow-tinted bright region indicates high-frequency energy increased. A blue-tinted region indicates it decreased. A region that goes from dark to bright with a shift in hue means both the level and spectral character of that moment changed.

**Interpreting the diff waveform:**

- Shows a bright amplitude-difference trace on a black background
- Areas where the waveform envelope changed appear as bright horizontal bands
- If the diff waveform shows dense bright content across the entire track, the gain change was large and uniform (expected from loudnorm)
- If the diff waveform shows isolated bright spikes, the limiter was catching specific transient peaks
- If the diff waveform is almost entirely black after a small uniform brightness: loudnorm applied a clean linear gain change with no dynamic processing — the ideal outcome

---

## The mastering report

Written to `<out_dir>/<stem>/<stem><suffix>_report.json`. Contains:

```json
{
  "generated_at": "2024-01-15T10:23:44Z",
  "input_file": "track.wav",
  "output_file": "mastered/track/track_master.wav",
  "preset": { ... full preset ... },
  "pre_analysis": {
    "integrated_lufs": -18.4,
    "true_peak_dbtp": -2.1,
    "loudness_range_lu": 14.6,
    "threshold_lufs": -28.7,
    "duration_secs": 463.2,
    "sample_rate": 44100,
    "channels": 2,
    "bit_depth": 24,
    "codec": "pcm_s24le",
    "extended": {
      "rms_dbfs": -22.1,
      "crest_factor_db": 20.0,
      "dynamic_range_dr": 17.0,
      "dc_offset": 0.00012,
      "phase_correlation": 0.74,
      "spectral_balance": {
        "low_pct": 38.2,
        "mid_pct": 44.1,
        "high_pct": 17.7
      },
      "spectral_centroid_hz": 1420.0
    }
  },
  "post_analysis": { ... same structure ... },
  "filter_chain": "highpass=f=20.0,alimiter=...,loudnorm=...",
  "delta": {
    "lufs_change": 2.4,
    "true_peak_change": 1.8,
    "lra_change": -0.3,
    "crest_factor_change": -0.2
  }
}
```

The `filter_chain` field contains the exact string passed to FFmpeg. A master is reproducible:

```sh
ffmpeg -i input.wav -af "<filter_chain>" -acodec pcm_s24le -ar 44100 output.wav
```

---

## Extending the pipeline

### Adding a filter stage

**1. Add the controlling field to `src/config.rs`:**

```rust
pub struct Filters {
    pub highpass_hz:   f32,
    pub lowpass_hz:    f32,
    pub high_shelf_db: f32,   // new
}
```

**2. Add a default in `impl Default for Preset`:**

```rust
filters: Filters {
    highpass_hz:   20.0,
    lowpass_hz:    0.0,
    high_shelf_db: 0.0,   // 0 = disabled by convention
},
```

**3. Push the filter into `build_filter_chain()` in `src/pipeline/process.rs`:**

```rust
if preset.filters.high_shelf_db != 0.0 {
    filters.push(format!("treble=g={:.1}:f=8000", preset.filters.high_shelf_db));
}
```

The push position in the vec determines where it runs in the chain.

**4. Add it to your preset `.toml`:**

```toml
[filters]
highpass_hz   = 20.0
lowpass_hz    = 0.0
high_shelf_db = 1.5
```

Rebuild: `cargo build --release`.

### Useful FFmpeg audio filters

Full reference: https://ffmpeg.org/ffmpeg-filters.html

```
# Parametric EQ band
equalizer=f=1000:width_type=o:width=2:g=-3.0

# High shelf
treble=g=2.0:f=8000

# Low shelf
bass=g=1.0:f=100

# Stereo widening
extrastereo=m=1.5

# Noise gate
agate=threshold=-40dB:ratio=2:attack=20:release=250

# Static gain
volume=2dB

# Resample
aresample=48000

# M/S encode (pan to mid/side)
pan=stereo|c0=0.5*c0+0.5*c1|c1=0.5*c0-0.5*c1
```

---

## AI integration

### matchering (reference-based mastering)

[matchering](https://github.com/sergree/matchering) analyzes a reference track and applies its spectral and loudness character to your track. Run it as a pre-pass.

```sh
pip install matchering

python3 << 'EOF'
import matchering as mg
mg.process(
    target='track.wav',
    reference='reference.wav',
    results=[mg.Result('track_matched.wav', use_limiter=False)]
)
EOF

mastacraf master track_matched.wav --preset default
```

Pass `use_limiter=False` to matchering. mastacraf's limiter handles the peak ceiling.

### demucs (source separation for analysis)

[demucs](https://github.com/facebookresearch/demucs) separates stems. Useful for inspecting the spectral and dynamic character of individual elements before mastering decisions.

```sh
pip install demucs
python3 -m demucs --two-stems=vocals track.wav
# examine separated/
mastacraf master track.wav --preset default
```

---

## Bundling (Tauri)

When wrapping in a Tauri desktop app:

**1. mastacraf as a Tauri sidecar** — in `src-tauri/tauri.conf.json`:

```json
"bundle": {
  "externalBin": ["../mastacraf/target/release/mastacraf"]
}
```

**2. FFmpeg as a sidecar** — use the `ffmpeg-sidecar` crate for automatic platform-specific FFmpeg download on first run:

```toml
ffmpeg-sidecar = "1.1"
```

Update `src/ffmpeg.rs` to check the sidecar path before the `$PATH` lookup.

**3. Presets** — the loader already searches `<exe_dir>/presets/`. Bundle your `.toml` files there.

**4. Frontend interface** — the frontend calls the sidecar, waits for exit, reads the JSON report from the output directory. No additional IPC is needed.

---

## Troubleshooting

**`ffmpeg not found`**  
FFmpeg is not in `$PATH` and not bundled next to the binary. Install it or copy the binary to the same directory as `mastacraf`.

**`Preset 'name' not found`**  
`presets/name.toml` does not exist in any search directory. Run `mastacraf presets` to see what is found. Confirm you are running from the directory containing `./presets/`.

**Post-master LRA is much lower than pre-master LRA**  
`target.lra` is set below the measured LRA. Loudnorm is compressing dynamics to fit. Raise `target.lra` to at or above the measured value and re-run.

**Phase correlation is negative or very low**  
There is a stereo phase issue in the mix. The mastering pipeline does not address phase problems. Fix at the mix stage before mastering.

**Extended analysis section is missing from output**  
The extended analysis passes run after loudnorm pass 1. If they fail (FFmpeg version missing a filter, or a very short file), the tool continues and marks extended analysis as absent. Run with `--verbose` to see which pass failed and why.

**Output louder or quieter than expected**  
Check the JSON report `delta.lufs_change`. If it does not match `target.lufs - pre_analysis.integrated_lufs`, the pass-1 measurements were not correctly parsed. Run with `--verbose` and inspect the raw loudnorm JSON in the FFmpeg stderr output.

**Spectrogram or waveform images missing**  
Requires FFmpeg compiled with `lavfi` virtual device support. Verify: `ffmpeg -filters | grep showspectrum`. Missing = your FFmpeg build is minimal. Install a full build.
