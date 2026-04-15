# mastercraft documentation

**Version:** 0.1.0  
**Repository:** your repo  
**License:** MIT

---

## Table of contents

1. [Overview](#overview)
2. [How it works](#how-it-works)
3. [Installation](#installation)
4. [Quick start](#quick-start)
5. [CLI reference](#cli-reference)
6. [Preset system](#preset-system)
7. [Preset field reference](#preset-field-reference)
8. [Loudness measurement](#loudness-measurement)
9. [The filter chain](#the-filter-chain)
10. [Visualization output](#visualization-output)
11. [The mastering report](#the-mastering-report)
12. [Extending the pipeline](#extending-the-pipeline)
13. [AI integration](#ai-integration)
14. [Bundling (Tauri)](#bundling-tauri)
15. [Troubleshooting](#troubleshooting)

---

## Overview

mastercraft is a command-line audio mastering pipeline built in Rust, using FFmpeg as its processing core. It is designed for mastering self-produced, self-mixed experimental electronic music where no commercial loudness standard applies and the goal is a repeatable, documented, and tunable process that you control entirely.

It is not a one-click mastering tool. It does not make aesthetic decisions for you. It applies the processing chain you define in a preset file, measures the result, and writes a report documenting exactly what happened.

**What it does:**

- Measures the loudness, true peak, and dynamic range of your input file
- Applies a configurable filter chain: high-pass, optional low-pass, optional compression, peak limiting, and two-pass EBU R128 loudness normalization
- Generates before/after spectrogram and waveform images
- Writes a JSON report containing all measurements, the exact FFmpeg filter string used, and the delta between input and output

**What it does not do:**

- Make EQ decisions
- Decide compression settings for you
- Automatically match loudness to a reference track
- Alter stereo field, phase, or any aspect of the mix outside the defined chain

---

## How it works

### The two-pass loudnorm process

The core of every master run is FFmpeg's `loudnorm` filter, run twice.

**Pass 1 (analysis):** The entire file is decoded and fed through loudnorm in measurement-only mode. No audio is written. The filter outputs a JSON block containing the file's measured integrated loudness (LUFS), true peak (dBTP), loudness range (LRA), and threshold. This takes approximately the same time as real-time playback.

**Pass 2 (processing):** The file is decoded again and fed through the full filter chain. The loudnorm filter on this pass receives the measurements from pass 1 as parameters and applies a linear gain adjustment to reach the target loudness. Because it has the full-file measurements, it can apply a precise, artifact-free gain change rather than estimating dynamically. The result is bit-accurate to the target.

This two-pass approach is the difference between a master that hits −16.0 LUFS reliably and one that is approximate. Single-pass loudnorm estimates in real time and will overshoot or undershoot depending on material.

### Filter chain execution

The filter chain is a comma-separated FFmpeg `-af` string. FFmpeg processes it as a linear graph: audio passes through each stage left to right, and the output of each stage is the input to the next. The chain is built in `src/pipeline/process.rs` by `build_filter_chain()`. Disabled stages are not included in the string — they do not exist in the graph and have zero processing overhead.

### Preset resolution

When you pass `--preset name`, the tool searches for `name.toml` in three directories in order, using the first match found:

1. `./presets/` relative to your current working directory
2. `<directory containing the mastercraft binary>/presets/`
3. `~/.config/mastercraft/presets/` (or the platform equivalent)

This means project-level presets (in `./presets/`) always take precedence over installed presets, which take precedence over user-global presets.

---

## Installation

### Prerequisites

- Rust toolchain: https://rustup.rs
- FFmpeg in your `$PATH`, or placed alongside the compiled binary

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

### Build from source

```sh
git clone <repo>
cd mastercraft
cargo build --release
```

The binary is at `./target/release/mastercraft`. Copy it to wherever you keep tools, or run it from the project directory.

### Verify

```sh
mastercraft --version
mastercraft --help
```

If FFmpeg is installed and on your PATH, that is all that is needed.

---

## Quick start

**Analyze a file before mastering:**

```sh
mastercraft analyze track.wav
```

This runs pass 1 only and prints the file's loudness measurements. No audio is written. Use this before choosing or writing a preset.

**Master with the default preset:**

```sh
mastercraft master track.wav
```

Output goes to `./mastered/`. You will get the mastered file, before/after images, and a JSON report.

**Master with a named preset:**

```sh
mastercraft master track.wav --preset noise
```

**Master without writing images (faster):**

```sh
mastercraft master track.wav --no-visualize
```

**Analyze without mastering, with images:**

```sh
mastercraft analyze track.wav --visualize --output ./analysis/
```

---

## CLI reference

### `mastercraft master`

Runs the full mastering pipeline: pre-analysis, pre-visualization, processing, post-analysis, post-visualization, report.

```
mastercraft master <INPUT> [OPTIONS]
```

| Argument / Flag      | Type    | Default        | Description |
|----------------------|---------|----------------|-------------|
| `<INPUT>`            | path    | required       | Input audio file. Any format FFmpeg can decode. |
| `-p`, `--preset`     | string  | `default`      | Name of preset to use. Searches `./presets/<name>.toml`. |
| `-o`, `--output`     | path    | `./mastered/`  | Output directory. Created if it does not exist. |
| `--suffix`           | string  | `_master`      | String appended to the input filename stem for the output file. |
| `--lufs`             | float   | from preset    | Override the integrated loudness target (LUFS). |
| `--true-peak`        | float   | from preset    | Override the true peak ceiling (dBTP). |
| `--no-visualize`     | flag    | off            | Skip spectrogram and waveform image generation. |
| `--dry-run`          | flag    | off            | Run pre-analysis and print measurements. Do not write audio or images. |
| `-v`, `--verbose`    | flag    | off            | Print every FFmpeg command as it executes. |

**Examples:**

```sh
# Basic master, all defaults
mastercraft master track.wav

# Different preset, custom output directory
mastercraft master track.wav -p film -o ./deliverables/

# Override loudness target without editing the preset
mastercraft master track.wav --lufs -14

# Verify what the pipeline would do without writing anything
mastercraft master track.wav -p noise --dry-run -v

# Custom filename suffix
mastercraft master track.wav --suffix _2024_v1
# → output: mastered/track_2024_v1.wav
```

---

### `mastercraft analyze`

Runs only pass 1 loudnorm measurement and optionally generates visualization images. No mastered file is written.

```
mastercraft analyze <INPUT> [OPTIONS]
```

| Argument / Flag    | Type | Default | Description |
|--------------------|------|---------|-------------|
| `<INPUT>`          | path | required | Input audio file. |
| `--visualize`      | flag | off | Generate spectrogram and waveform images. |
| `-o`, `--output`   | path | `./`    | Directory for visualization images (only used with `--visualize`). |
| `-v`, `--verbose`  | flag | off | Print FFmpeg commands. |

**Output printed to stdout:**

```
  [analysis]
    integrated loudness    -18.4 LUFS
    true peak              -1.2 dBTP
    loudness range         14.6 LU
    duration               00:07:43
    format                 pcm_s24le / 44100 Hz / 2 ch
```

---

### `mastercraft presets`

Lists all presets found across all search directories. Prints name, LUFS target, true peak, LRA, and description.

```sh
mastercraft presets
```

---

### `mastercraft preset <NAME>`

Prints the full parsed content of a named preset to stdout. Useful for inspecting what is loaded and for copying as a starting point.

```sh
mastercraft preset noise
mastercraft preset film > mypreset.toml   # copy as starting point
```

---

## Preset system

### What a preset is

A preset is a TOML file that fully defines one mastering configuration. It contains loudness targets, filter settings, output format, and visualization settings. Every field that controls audio processing maps directly to an FFmpeg filter parameter — there is no additional logic applied by the tool.

### File location

Place preset files in `./presets/` relative to where you run the `mastercraft` command. This is the first location searched and the one you should use for per-project presets.

```
your-project/
  presets/
    default.toml
    noise.toml
    film.toml
    thisalbum.toml     ← your custom preset
  tracks/
    track01.wav
```

### Creating a preset

The recommended workflow:

**Step 1: Analyze your material.**

```sh
mastercraft analyze track.wav
```

Note the reported LUFS, LRA, and true peak values. These are your baseline. They tell you what the material actually measures and how far the pipeline needs to move it.

**Step 2: Decide on targets based on where the file is going.**

See the loudness targets table in the cheat sheet. If you do not know the destination, -16 LUFS / -1 dBTP / LRA at or above measured is a safe default for experimental music.

**Step 3: Copy a preset and edit it.**

```sh
cp presets/default.toml presets/mypreset.toml
```

Edit `mypreset.toml`. Change `[meta].name` to match the filename. Set `[target]` values based on step 2. Decide whether the compressor should be enabled.

**Step 4: Test without writing.**

```sh
mastercraft master track.wav -p mypreset --dry-run
```

This runs the full analysis and prints what the pre-master measurements are. You are not committing to anything yet.

**Step 5: Run and check the report.**

```sh
mastercraft master track.wav -p mypreset
cat mastered/track_master_report.json
```

The report contains pre and post measurements and a delta. If the post-master LRA is significantly lower than your input LRA, your `target.lra` is set too low and loudnorm is squeezing the dynamics to fit. Raise it.

### The `[meta]` section

```toml
[meta]
name        = "mypreset"     # must match the filename stem
description = "..."          # shown in `mastercraft presets` output
author      = ""             # optional
notes       = ""             # freeform, for your own reference
```

`name` is used only for display. It does not need to match the filename, but keeping them consistent avoids confusion.

---

## Preset field reference

### `[target]`

#### `lufs` (float, negative)

Integrated loudness target in LUFS. The loudnorm filter adjusts the overall gain of the file so that the integrated loudness measures at this value.

**Range:** −32.0 to −9.0  
**Default:** −16.0

The relationship between this value and what you hear: lower values produce a quieter master with more headroom. Higher values produce a louder master. The loudness target does not affect the relative dynamics within the track — it shifts the overall gain. The compressor and LRA target affect dynamics.

LUFS is a perceived loudness measurement, not a peak measurement. A track with large dynamic range will have a lower integrated LUFS than a track of the same peak level with compressed dynamics, because the quiet passages pull the average down.

Do not set this to the maximum streaming target if your material has wide dynamics. If your material has 20 dB of dynamic range and you target −9 LUFS, the loud sections will hit 0 dBFS and the limiter will be working extremely hard. Target a level where your loudest passages hit the limiter occasionally, not continuously.

#### `true_peak` (float, negative)

Ceiling for intersample peaks in dBTP. Both the limiter and loudnorm enforce this ceiling.

**Range:** −3.0 to −0.1  
**Default:** −1.0

Intersample peaks occur when the DAC reconstructs the waveform between samples. A file where no sample exceeds 0 dBFS can still clip on playback because the interpolated waveform between samples may exceed 0. True peak measurement accounts for this by oversampling (typically 4x) before measuring peaks.

−1.0 dBTP is the delivery standard for most streaming platforms and distribution services. −2.0 dBTP is the EBU R128 broadcast standard. Never set this to 0.0 — intersample clipping will occur on some playback systems.

#### `lra` (float, positive)

Loudness range target in Loudness Units. This is the target spread between the quiet and loud sections of the track as measured by the loudnorm filter.

**Range:** 1.0 to 20.0  
**Default:** 11.0

This is the most important value to set correctly for experimental music. The loudnorm filter attempts to fit the material's dynamic range into this target. If the material's natural LRA exceeds the target, loudnorm will apply dynamic range compression to make it fit. If the target is at or above the material's natural LRA, loudnorm applies only a linear gain adjustment and dynamics are fully preserved.

**Rule: set `lra` at or above the value reported by `mastercraft analyze`.**

If you have a film score that naturally spans 22 LU between silence and full orchestra, and you set `lra = 8`, loudnorm will quietly compress 14 LU of your dynamic range away. There is no warning. The only way to catch this is to compare the LRA in the post-master report against the pre-master measurement.

---

### `[filters]`

#### `highpass_hz` (float)

Removes all audio below this frequency using a first-order highpass filter.

**Range:** 0 (disabled) to 120.0  
**Default:** 20.0

Content below 20 Hz is inaudible on any speaker or headphone. However, it contributes to the energy measured by loudnorm and occupies headroom in the limiter. A 20 Hz highpass is transparent to all audible content while removing DC offset and infrasonic noise.

For abstract and noise music where sub-bass is compositional, lower this. At 12 Hz you retain all audible bass while removing genuine DC offset and mechanical noise that might have entered through recording equipment. Below 12 Hz the filter is operating well outside audible range and you are keeping content that no speaker will reproduce.

Set to 0 to disable entirely.

#### `lowpass_hz` (float)

Removes all audio above this frequency.

**Range:** 0 (disabled) to 24000.0  
**Default:** 0.0 (disabled)

Not needed for standard mastering. Relevant only for specific deliverable requirements (broadcast with frequency ceilings, telephone, AM radio) or if you are deliberately shaping the high-frequency character of a master. If you are using this, you have a specific reason for it.

---

### `[compressor]`

#### `enabled` (bool)

**Default:** `false`

The compressor processes the entire stereo bus simultaneously. It responds to the sum of all elements in the mix. On most well-mixed material, enabling it changes the relationship between elements in ways that cannot be undone. The default is `false` because you already made dynamic decisions in the mix.

Enable it only when you have a specific reason: the mix has inconsistent section-to-section loudness you want to even out, you want to add density and glue, or you know the material well enough to set the parameters intentionally.

#### `threshold_db` (float, negative)

Level above which compression engages.

**Range:** −60.0 to 0.0  
**Default:** −18.0

Everything above the threshold gets reduced by the ratio. Everything below is untouched. Setting the threshold lower means the compressor engages on more of the material. Setting it higher means it only catches the loudest peaks.

For a mastering context on wide-range material, a threshold around −18 to −12 dB engages on the louder sections while leaving quieter passages fully intact.

#### `ratio` (float)

The amount of gain reduction applied to signal above the threshold.

**Range:** 1.0 (no reduction) to 20.0  
**Default:** 2.0

At 2:1, every 2 dB over the threshold becomes 1 dB in the output. At 4:1, every 4 dB becomes 1 dB. For mastering: 1.5:1 to 2:1 is transparent-to-gentle. 4:1 is noticeable. Above 8:1 is limiting territory and you should use the limiter stage instead.

#### `attack_ms` (float, milliseconds)

How quickly the compressor responds when signal exceeds the threshold.

**Range:** 0.1 to 200.0  
**Default:** 20.0

Fast attack (1–5ms) catches the initial transient. Slow attack (20–80ms) lets the transient through before the compressor clamps down. For abstract and noise music with intentionally designed transients, slow attack preserves the attack shape of sounds. Fast attack flattens it.

The attack time also affects how the compressor interacts with low frequencies. A very fast attack on bass-heavy material compresses within a single waveform cycle, causing distortion. Keep attack above 5ms for material with significant sub-bass content.

#### `release_ms` (float, milliseconds)

How quickly the compressor stops compressing after signal drops below the threshold.

**Range:** 10.0 to 2000.0  
**Default:** 250.0

Fast release (50ms) causes the gain to snap back quickly, which can create pumping artifacts on sustained dense material. Slow release (250–500ms) is more transparent because the gain reduction fades out gradually. For material with long sustained textures, slower release sounds more natural.

#### `makeup_db` (float)

Gain added after compression to compensate for level reduction.

**Range:** 0.0 to 24.0  
**Default:** 0.0

The loudnorm stage after the compressor will normalize the final integrated loudness to `target.lufs` regardless of this value. Makeup gain here shifts where the compressor is operating relative to the threshold, not the final output level. If you add 3 dB of makeup, you are effectively lowering the threshold by 3 dB — more material will exceed it and be compressed.

#### `knee_db` (float)

Width of the soft-knee transition zone around the threshold.

**Range:** 0.0 to 12.0  
**Default:** 2.0

At 0, the compressor switches on instantly at the threshold (hard knee). At 6, it begins applying partial compression at `threshold − 3 dB` and reaches full ratio at `threshold + 3 dB`. Soft knee sounds more gradual and natural on material without a conventional rhythmic structure.

---

### `[limiter]`

#### `enabled` (bool)

**Default:** `true`

The limiter prevents the output from exceeding `target.true_peak`. Keep this enabled. The only reason to disable it is if you are certain your material never exceeds the true peak ceiling and you want to skip the processing overhead — which is negligible.

#### `attack_ms` (float, milliseconds)

How fast the limiter responds to a peak.

**Range:** 0.1 to 20.0  
**Default:** 5.0

Fast attack (0.5–1ms) catches all peaks including hard impulsive transients. It can introduce small amounts of distortion on low-frequency peaks because it cuts into the waveform cycle. Slow attack (5–10ms) is more transparent but may allow brief intersample overs on very hard transients before the limiter responds.

For noise and harsh electronic material with frequent hard transients: use 0.5–1ms. For film scoring material where transient shape (the attack of a percussion hit, the bow attack of a string section) is musically important: use 5–10ms and accept that occasional peaks may slightly exceed the ceiling before the limiter engages.

#### `release_ms` (float, milliseconds)

How fast the limiter recovers after a peak.

**Range:** 10.0 to 500.0  
**Default:** 50.0

Short release (20–50ms) lets the limiter recover quickly. On dense material with many consecutive peaks, short release means the limiter is frequently engaging and disengaging, which can create a pumping texture. Long release (100–300ms) smooths this out but can hold the gain reduction into subsequent quieter passages.

---

### `[output]`

#### `format` (string)

Output file format.

**Values:** `wav` | `flac` | `mp3` | `aac`  
**Default:** `wav`

For archival and delivery masters, use `wav` or `flac`. WAV is uncompressed. FLAC is lossless compressed (smaller file, identical quality). MP3 and AAC are lossy — use only for platform-specific delivery requirements where an uncompressed master already exists.

#### `bit_depth` (integer)

PCM bit depth. Applies to `wav` and `flac` only.

**Values:** `16` | `24` | `32`  
**Default:** `24`

24-bit is the standard for mastering. 16-bit is CD standard — use only if the deliverable specifically requires it (CD replication). 32-bit float is useful for intermediate files going into another DAW for further processing.

#### `sample_rate` (integer)

Output sample rate in Hz.

**Values:** any FFmpeg-supported rate; common: `44100` | `48000` | `96000`  
**Default:** `44100`

44100 Hz is the CD and streaming standard. 48000 Hz is the post-production and broadcast standard — use for any material going into a film or video pipeline. 96000 Hz is for high-resolution delivery or intermediate files. Do not upsample — if the input is 44100 Hz, setting this to 96000 produces a 96000 Hz file that contains no additional information.

---

### `[visualization]`

#### `enabled` (bool)

Master switch for all image generation.

**Default:** `true`

Set to `false` to skip both spectrogram and waveform images for all runs with this preset. Can also be overridden per-run with `--no-visualize`.

#### `spectrogram` (bool)

Generate spectrogram image.

**Default:** `true`

Uses FFmpeg's `showspectrumpic` filter with logarithmic frequency scale and intensity color map. Logarithmic scale is used because it distributes the frequency axis proportionally to how hearing works — each octave gets equal visual space. Linear scale would compress most of the interesting content into the bottom 10% of the image.

#### `waveform` (bool)

Generate waveform image.

**Default:** `true`

Uses FFmpeg's `showwavespic` filter. Channels are split (L on top, R below) with distinct colors so stereo content is readable. Useful for checking for clipping, seeing the envelope shape, and comparing pre/post dynamic range visually.

#### `width` (integer, pixels)

Horizontal resolution of visualization images.

**Default:** 1920

Both the spectrogram and waveform images are rendered at this width. 1920 gives one pixel per ~2ms for a 60-minute track. Increase to 3840 or 7680 for very long pieces where you need time resolution.

#### `spectrogram_height` / `waveform_height` (integer, pixels)

Vertical resolution of each image type.

**Defaults:** 512 / 200

For content with dense spectral activity across a wide frequency range, increase `spectrogram_height` to 768 or 1024. More pixels = more frequency resolution in the image. The waveform height is a matter of preference — 200px is readable.

---

## Loudness measurement

### What `mastercraft analyze` reports

**Integrated loudness (LUFS):** The average perceived loudness of the entire track, computed over all non-silent sections using the ITU-R BS.1770 algorithm. This is the value that streaming platforms use for normalization.

**True peak (dBTP):** The maximum intersample peak across the entire track, measured at 4x oversampling.

**Loudness range (LU):** The statistical spread between quiet and loud sections (short-term loudness, 10th to 95th percentile). A classical piece might measure 20+ LU. A wall of noise might measure 3 LU.

**Threshold (LUFS):** The loudness gate threshold used during measurement. Sections below this level are excluded from the integrated loudness calculation. This is why near-silent intros or outros do not pull the measured LUFS down dramatically.

### Using measurements to set preset values

Run `mastercraft analyze` before writing a preset. Use the output as follows:

- `integrated loudness` → shows how much the loudnorm gain will be adjusted. If measured is −22 and target is −16, loudnorm adds 6 dB of gain, which also raises the true peak by 6 dB. Ensure the limiter headroom accommodates this.
- `loudness range` → set `target.lra` at or above this value. If measured LRA is 14 LU and you set target to 8 LU, loudnorm compresses 6 LU of dynamics away.
- `true peak` → tells you how hard the limiter will work. If measured TP is −3 dBTP and target TP is −1, the limiter needs to attenuate 2 dB of peaks. If measured TP is −0.2 and target is −1, the limiter is working extremely hard on most peaks.

---

## The filter chain

### Structure

The filter chain is built as a comma-separated FFmpeg `-af` string by `build_filter_chain()` in `src/pipeline/process.rs`. The order is fixed:

```
highpass → lowpass → compressor → limiter → loudnorm
```

Disabled stages are omitted entirely. A preset with `highpass_hz = 20`, `compressor.enabled = false`, `limiter.enabled = true` produces:

```
highpass=f=20.0,alimiter=level_in=1:level_out=0.891251:limit=0.891251:attack=5.0:release=50.0:asc=1,loudnorm=I=-16.0:TP=-1.0:LRA=11.0:measured_I=-18.43:...
```

### Viewing the chain used for a master

The `--verbose` flag prints the FFmpeg command as it runs, including the full filter string. The mastering report also saves it:

```sh
mastercraft master track.wav -v
cat mastered/track_master_report.json | grep filter_chain
```

### The `alimiter` true peak conversion

The limiter's `level_out` parameter takes a linear amplitude value, not dBTP. The tool converts `target.true_peak` from dBTP to linear automatically:

```
linear = 10 ^ (true_peak / 20)
-1.0 dBTP  → 0.891251
-0.5 dBTP  → 0.944061
-2.0 dBTP  → 0.794328
```

You set `true_peak` in dBTP in the preset. The conversion is handled internally.

---

## Visualization output

Two images are generated per run: one before mastering (tagged `_pre_`) and one after (`_post_`). Comparing them visually is the primary way to verify the master behaved as expected.

### Spectrogram (`_spectrogram.png`)

Generated with `showspectrumpic=mode=combined:color=intensity:scale=log:legend=1`.

- **X axis:** time (left = start, right = end)
- **Y axis:** frequency (log scale, bottom = low, top = high)
- **Color:** intensity (darker = louder at that frequency and time)
- **Legend:** frequency axis labels are printed on the right edge

Use the spectrogram to check: where the spectral energy lives, whether the highpass is removing content you did not intend, whether the high end is preserved or rolled off, and whether the overall character changed between pre and post.

### Waveform (`_waveform.png`)

Generated with `showwavespic=split_channels=1`.

- **X axis:** time
- **Y axis:** amplitude (0 at center, ±1 at top/bottom)
- **Channels:** L on top half, R on bottom half, in different colors

Use the waveform to check: whether the dynamics are preserved (the envelope shape should be similar between pre and post), whether clipping is occurring (flat tops on peaks), and the overall density of the material.

---

## The mastering report

Every master run writes a JSON file to the output directory. It contains:

```json
{
  "generated_at": "2024-01-15T10:23:44Z",
  "input_file": "track.wav",
  "output_file": "mastered/track_master.wav",
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
    "codec": "pcm_s24le"
  },
  "post_analysis": { ... same structure ... },
  "filter_chain": "highpass=f=20.0,alimiter=...,loudnorm=...",
  "delta": {
    "lufs_change": 2.4,
    "true_peak_change": 1.8,
    "lra_change": -0.3
  }
}
```

The `filter_chain` field contains the exact string passed to FFmpeg `-af`. A master is reproducible by taking this string and running:

```sh
ffmpeg -i input.wav -af "<filter_chain>" -acodec pcm_s24le -ar 44100 output.wav
```

The report is the permanent record of what was done. Keep it alongside the mastered file.

---

## Extending the pipeline

### Adding a filter stage

To add a new filter (EQ, stereo width, de-esser, etc.):

**1. Add the field to the config struct in `src/config.rs`:**

```rust
pub struct Filters {
    pub highpass_hz:   f32,
    pub lowpass_hz:    f32,
    pub high_shelf_db: f32,   // ← new field
}
```

**2. Add a default value in the `Default` implementation:**

```rust
filters: Filters {
    highpass_hz:   20.0,
    lowpass_hz:    0.0,
    high_shelf_db: 0.0,   // ← 0 = disabled by convention
},
```

**3. Add the filter to `build_filter_chain()` in `src/pipeline/process.rs`:**

```rust
if preset.filters.high_shelf_db != 0.0 {
    filters.push(format!(
        "treble=g={:.1}:f=8000",
        preset.filters.high_shelf_db
    ));
}
```

Position this push at the point in the vec where you want it to run in the chain.

**4. Add it to your preset `.toml`:**

```toml
[filters]
highpass_hz   = 20.0
lowpass_hz    = 0.0
high_shelf_db = 1.5   # gentle high shelf boost
```

That is the complete change. Rebuild with `cargo build --release`.

### Useful FFmpeg audio filters for this context

All filters: https://ffmpeg.org/ffmpeg-filters.html#Audio-Filters

**EQ:**
```
equalizer=f=1000:width_type=o:width=2:g=-3.0    # parametric EQ band
treble=g=2.0:f=8000                              # high shelf
bass=g=1.0:f=100                                 # low shelf
```

**Stereo:**
```
extrastereo=m=1.5                                # stereo width
pan=stereo|c0=0.5*c0+0.5*c1|c1=0.5*c0-0.5*c1   # M/S encode
```

**Dynamics:**
```
agate=threshold=-40dB:ratio=2:attack=20:release=250   # noise gate
```

**Utility:**
```
aresample=44100                                  # resample
volume=2dB                                       # static gain
```

### Adding a configurable preset field

If the FFmpeg parameter is a float that users will want to tune per-preset, add it to the appropriate config struct and `.toml` section following the pattern above. If it is a string (e.g. a filter type), use `String` in the struct and parse/match it when building the filter string in `process.rs`.

---

## AI integration

### matchering (reference-based)

[matchering](https://github.com/sergree/matchering) is an open-source Python library that analyzes a reference track and applies its spectral and loudness character to your track. It operates as a pre-pass before mastercraft's filter chain.

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

mastercraft master track_matched.wav --preset default
```

Pass `use_limiter=False` to matchering — mastercraft's limiter handles the peak ceiling.

### ONNX / demucs (source separation as pre-pass)

[demucs](https://github.com/facebookresearch/demucs) can separate a mix into stems. For mastering experimental music this is less about separation and more about analysis — checking what the stem decomposition reveals about the spectral balance before committing to EQ decisions.

```sh
pip install demucs
python3 -m demucs --two-stems=vocals track.wav
# examine output in separated/ directory
# then master the original
mastercraft master track.wav --preset default
```

### Future `--ai-preprocess` flag

The pipeline in `src/pipeline/mod.rs` currently calls `process::run_process()` directly after analysis. A future version will insert an optional pre-processing step between analysis and processing, accepting an external command or Python script path via `--ai-preprocess <script>`. The analyzed measurements will be passed to the script as JSON on stdin, and the script will write a processed intermediate file that the mastering chain then operates on.

---

## Bundling (Tauri)

When the tool is ready to be wrapped in a desktop UI via Tauri:

**1. Compile mastercraft as a Tauri sidecar.**

Add to `src-tauri/tauri.conf.json`:

```json
"bundle": {
  "externalBin": ["../mastercraft/target/release/mastercraft"]
}
```

Tauri will bundle the binary into the app package. The binary is called from the frontend via `@tauri-apps/api/shell`.

**2. Bundle FFmpeg as a second sidecar, or use `ffmpeg-sidecar`.**

The `ffmpeg-sidecar` crate handles downloading a platform-appropriate FFmpeg binary on first run:

```toml
# Add to mastercraft/Cargo.toml
ffmpeg-sidecar = "1.1"
```

Update `src/ffmpeg.rs` to check for the sidecar-managed binary path before the PATH lookup.

**3. Preset resolution already supports bundled installs.**

The preset loader searches `<exe_dir>/presets/` which, in a bundled Tauri app, will be inside the app bundle. Bundle your preset `.toml` files there.

**4. The JSON report is the interface.**

The Tauri frontend reads the JSON report to display measurements and results. The frontend calls the sidecar, waits for it to exit, reads the report from the output directory. No IPC beyond filesystem reads is needed for the initial UI.

---

## Troubleshooting

**`ffmpeg not found`**

FFmpeg is not in your PATH and is not bundled alongside the binary. Install it with your package manager or place the ffmpeg binary in the same directory as the mastercraft binary.

**`Preset 'name' not found`**

The file `presets/name.toml` does not exist in any of the search directories. Run `mastercraft presets` to see what is found. Verify you are running from the directory containing `./presets/`.

**`No JSON found in loudnorm output`**

FFmpeg ran but the loudnorm filter did not produce its JSON output block. This usually means the input file could not be decoded (corrupted, unsupported format) or the filter name has changed in your FFmpeg version. Run with `--verbose` to see the exact FFmpeg command and run it manually to see the raw stderr output.

**Post-master LRA is much lower than pre-master LRA**

The `target.lra` in your preset is set lower than the measured LRA of the input. Loudnorm is compressing the dynamic range to fit. Raise `target.lra` to at or above the value reported by `mastercraft analyze`.

**Spectrogram or waveform images are blank or missing**

The visualization filters (`showspectrumpic`, `showwavespic`) require FFmpeg to be compiled with the `lavfi` virtual device support. Most standard FFmpeg distributions include this. Run `ffmpeg -filters | grep showspectrum` to verify. If it is missing, you need a fuller FFmpeg build.

**Output file is louder/quieter than expected**

Run `mastercraft analyze` on both the input and output files and compare the integrated LUFS values. If the output LUFS does not match `target.lufs`, check the JSON report's `filter_chain` field to confirm loudnorm is receiving the correct `measured_I` value from pass 1. Mismatches here indicate the pass-1 JSON parsing failed silently — run with `--verbose` and examine the raw loudnorm output in stderr.
