# mastercraft — cheat sheet

## Commands

```sh
mastercraft master <file>                        # master with default preset
mastercraft master <file> -p noise               # use a named preset
mastercraft master <file> -p film -o ./out/      # specify output directory
mastercraft master <file> --lufs -18             # override loudness target
mastercraft master <file> --true-peak -0.5       # override peak ceiling
mastercraft master <file> --suffix _v2           # custom output filename suffix
mastercraft master <file> --no-visualize         # skip spectrogram/waveform
mastercraft master <file> --dry-run              # analyze only, write nothing
mastercraft master <file> -v                     # verbose: print ffmpeg commands

mastercraft analyze <file>                       # measure loudness, print stats
mastercraft analyze <file> --visualize           # also generate images
mastercraft analyze <file> --visualize -o ./viz/ # images to specific directory

mastercraft presets                              # list all available presets
mastercraft preset noise                         # print full preset to stdout
```

---

## Output (per master run)

```
mastered/
  <stem>_master.wav              mastered audio
  <stem>_pre_spectrogram.png     input spectrogram (log scale, full duration)
  <stem>_pre_waveform.png        input waveform (L/R split)
  <stem>_post_spectrogram.png    output spectrogram
  <stem>_post_waveform.png       output waveform
  <stem>_master_report.json      full report: measurements, filter chain, delta
```

---

## Preset fields — all values

```toml
[target]
lufs      = -16.0    # integrated loudness. range: -32 to -9. lower = more dynamics
true_peak = -1.0     # intersample peak ceiling in dBTP. range: -3.0 to -0.1
lra       = 11.0     # loudness range in LU. range: 1–20. higher = more dynamics

[filters]
highpass_hz = 20.0   # remove below this Hz. 0 = disabled. range: 0–120
lowpass_hz  = 0.0    # remove above this Hz. 0 = disabled. rarely needed

[compressor]
enabled      = false  # true/false. off by default — trust the mix
threshold_db = -18.0  # engage above this level. range: -60 to 0
ratio        = 2.0    # reduction ratio. 1.0 = off, 2.0 = gentle, 10.0 = limiting
attack_ms    = 20.0   # response time in ms. slow = preserve transients
release_ms   = 250.0  # recovery time in ms. slow = transparent
makeup_db    = 0.0    # post-compression gain. loudnorm controls final level
knee_db      = 2.0    # soft-knee width. 0 = hard, 6 = very gradual

[limiter]
enabled    = true     # true/false. almost always on
attack_ms  = 5.0      # ms. fast (0.5) = catch all peaks, slow (10) = preserve shape
release_ms = 50.0     # ms. short = recover fast, long = smoother on dense material

[output]
format      = "wav"   # wav | flac | mp3 | aac
bit_depth   = 24      # 16 | 24 | 32  (wav/flac only)
sample_rate = 44100   # 44100 | 48000 | 96000

[visualization]
enabled            = true
spectrogram        = true
waveform           = true
width              = 1920   # image width in px
spectrogram_height = 512    # px. increase for dense spectral content
waveform_height    = 200    # px
```

---

## Filter chain order (fixed)

```
highpass → lowpass → compressor → limiter → loudnorm (2-pass)
```

Disabled stages are skipped entirely. Order cannot be changed without editing `src/pipeline/process.rs`.

---

## Loudness targets by destination

| Destination          | lufs   | true_peak | lra      |
|----------------------|--------|-----------|----------|
| Spotify              | -14.0  | -1.0      | your call|
| Apple Music          | -16.0  | -1.0      | your call|
| YouTube              | -14.0  | -1.0      | your call|
| Bandcamp / archive   | -16.0  | -1.0      | your call|
| Film / broadcast     | -23.0  | -2.0      | 20.0     |
| Vinyl prep           | -12.0  | -0.3      | 12.0     |
| DJ pool              | -9.0   | -0.5      | 8.0      |
| Personal / no norm.  | -18.0  | -1.0      | 18.0+    |

---

## Preset location resolution order

```
1. ./presets/<name>.toml          (working directory — use this)
2. <exe dir>/presets/<name>.toml  (bundled install)
3. ~/.config/mastercraft/presets/ (user config dir)
```

---

## Create a new preset

```sh
cp presets/default.toml presets/mypreset.toml
# edit mypreset.toml
mastercraft preset mypreset           # verify it loaded
mastercraft master track.wav -p mypreset --dry-run   # test without writing
mastercraft master track.wav -p mypreset
```

---

## Analyze before setting preset values

```sh
mastercraft analyze track.wav
```

Use the reported values to set targets:
- reported LUFS → tells you how far the loudnorm will push the gain
- reported LRA  → set `target.lra` at or above this value
- reported TP   → tells you how hard the limiter will work

---

## Add a filter stage (EQ, stereo width, etc.)

In `src/pipeline/process.rs`, inside `build_filter_chain()`, push a new filter string into the `filters` vec at the desired position. FFmpeg filter reference: https://ffmpeg.org/ffmpeg-filters.html

```rust
// Example: high shelf boost at 8kHz
filters.push("treble=g=2.0:f=8000".to_string());

// Example: stereo widening
filters.push("extrastereo=m=2.0".to_string());

// Example: mid/side EQ (requires split/merge)
filters.push("pan=stereo|c0=0.5*c0+0.5*c1|c1=0.5*c0-0.5*c1".to_string());
```

Add the controlling field to the `Filters` struct in `src/config.rs` and to your preset `.toml` to make it configurable.
