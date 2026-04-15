<p align="center">
  <img width="300px" height="950px" src="https://github.com/schwwaaa/mastacraf/blob/main/assets/schwwaaa-mastacraf-logo.png?raw=true"/>  
</p>

<p align="center">
  <strong>Custom audio mastering pipeline for experimental electronic music.</strong><br/>
  <strong>Built around FFmpeg. Designed to be yours.</strong><br/>  
</p>

---

## Install

**Prerequisites:** Rust toolchain + FFmpeg on your PATH.

```sh
# macOS
brew install ffmpeg rust

# Ubuntu / Debian
sudo apt install ffmpeg
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Build:**

```sh
git clone <your repo>
cd mastacraf
cargo build --release
# Binary: ./target/release/mastacraf
```

---

## Usage

### Master a file

```sh
mastacraf master track.wav
mastacraf master track.wav --preset noise
mastacraf master track.wav --preset film --output ./deliverables/
mastacraf master track.wav --lufs -18 --true-peak -0.5
```

**Options:**

| Flag              | Default      | Description                          |
|-------------------|--------------|--------------------------------------|
| `--preset`        | `default`    | Preset name (see `presets/`)         |
| `--output`        | `./mastered/`| Output directory                     |
| `--suffix`        | `_master`    | Appended to output stem              |
| `--lufs`          | from preset  | Override integrated loudness target  |
| `--true-peak`     | from preset  | Override true peak ceiling           |
| `--no-visualize`  | off          | Skip spectrogram / waveform images   |
| `--dry-run`       | off          | Analyze only, write nothing          |
| `--verbose` / `-v`| off          | Print ffmpeg commands                |

### Analyze only

```sh
mastacraf analyze track.wav
mastacraf analyze track.wav --visualize
```

### Preset management

```sh
mastacraf presets            # list all available presets
mastacraf preset noise       # dump noise.toml to stdout
```

---

## Output

For each mastered track you get:

```
mastered/
  track_master.wav             # mastered file
  track_pre_spectrogram.png    # input spectrogram
  track_pre_waveform.png       # input waveform
  track_post_spectrogram.png   # output spectrogram
  track_post_waveform.png      # output waveform
  track_master_report.json     # full mastering report
```

The JSON report contains all loudness measurements, the exact filter chain
used, pre/post comparison, and preset settings — so a master is reproducible.

---

## Presets

Presets live in `./presets/` (relative to your working directory). Copy and
modify any `.toml` file to create a new preset. Run `mastacraf presets` to
confirm it's found.

Included presets:

| Preset    | LUFS  | TP     | LRA   | For                                  |
|-----------|-------|--------|-------|--------------------------------------|
| `default` | −16.0 | −1 dBTP| 11 LU | General experimental / abstract      |
| `noise`   | −20.0 | −0.5   | 18 LU | Harsh noise / power electronics      |
| `film`    | −23.0 | −2.0   | 20 LU | Film scoring / cinematic / broadcast |

Create as many as you need. The preset system is designed for per-aesthetic
tuning — you might have `ambient.toml`, `drone.toml`, `release.toml`, etc.

---

## AI Integration

### matchering (reference-based mastering)

[matchering](https://github.com/sergree/matchering) is an open-source Python
library that analyzes a reference track and matches your mix to it. It works
well as a pre-pass before running mastacraf's loudnorm/limiting stage.

```sh
pip install matchering

# Pre-process with a reference before mastering:
python3 -c "
import matchering as mg
mg.process(
    target='track.wav',
    reference='reference.wav',
    results=[mg.Result('track_matched.wav', use_limiter=False)]
)
"

# Then master the matched output:
mastacraf master track_matched.wav --preset default
```

### Local ONNX models

For spectral enhancement or denoising before mastering, ONNX models can be
run via [onnxruntime](https://onnxruntime.ai) in a Python preprocessing step.
Models like `demucs` (source separation) can be used to inspect or isolate
elements before the final master.

### Future AI hook

A `--ai-preprocess` flag is planned in a future version to chain a Python/ONNX
preprocessing step inline with the mastering pipeline. The architecture is
already split so `pipeline/process.rs` can be preceded by an arbitrary step.

---

## Bundling (future Tauri path)

When you're ready to wrap this in a Tauri app:

1. Compile the Rust binary as a Tauri sidecar:
   ```toml
   # tauri.conf.json
   "bundle": { "externalBin": ["../mastacraf/target/release/mastacraf"] }
   ```

2. Bundle the FFmpeg binary as a second sidecar (or use `ffmpeg-sidecar` crate
   for automatic platform-specific FFmpeg download on first run).

3. The preset system already supports loading from the executable directory,
   so bundled presets will resolve automatically.

---

## Extending

The pipeline is intentionally linear and transparent:

```
analyze() → [visualize(pre)] → process() → [visualize(post)] → report()
```

Each stage is its own module in `src/pipeline/`. To add a stage (e.g. a
mid/side matrix step, a custom EQ curve, or an AI pass), add a module and
wire it in `src/pipeline/mod.rs`.

The filter chain is built as a plain string in `process::build_filter_chain()`.
FFmpeg's audio filter graph is extremely deep — browse
https://ffmpeg.org/ffmpeg-filters.html for anything you need.
