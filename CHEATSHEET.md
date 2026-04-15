# mastercraft — cheat sheet

## Commands

```sh
mastercraft master <file>                        # master with default preset
mastercraft master <file> -p noise               # use a named preset
mastercraft master <file> -p film -o ./out/      # specify base output directory
mastercraft master <file> --lufs -18             # override loudness target
mastercraft master <file> --true-peak -0.5       # override peak ceiling
mastercraft master <file> --suffix _v2           # custom output filename suffix
mastercraft master <file> --no-visualize         # skip spectrogram/waveform
mastercraft master <file> --dry-run              # analyze only, write nothing
mastercraft master <file> -v                     # verbose: print ffmpeg commands

mastercraft analyze <file>                       # full analysis including extended stats
mastercraft analyze <file> --visualize           # also generate images
mastercraft analyze <file> --visualize -o ./viz/ # images to specific directory

mastercraft presets                              # list all available presets
mastercraft preset noise                         # print full preset to stdout
```

---

## Output folder structure (per track)

Each master run creates a dedicated subfolder named after the input file stem:

```
mastered/
  track01/
    track01_master.wav

    track01_pre_spectrogram.png       individual pre spectrogram
    track01_pre_waveform.png          individual pre waveform
    track01_post_spectrogram.png      individual post spectrogram
    track01_post_waveform.png         individual post waveform

    track01_compare_spectrogram.png   pre + post stacked (amber separator)
    track01_compare_waveform.png      pre + post stacked (amber separator)
    track01_diff_spectrogram.png      amplified pixel difference — where things changed
    track01_diff_waveform.png         amplified pixel difference

    track01_master_report.json
```

With a custom base directory: `mastercraft master track.wav -o ./album/`  
→ output goes to `./album/track/`

---

## Analysis output

### Core (EBU R128)
| Field              | What it means                                              |
|--------------------|------------------------------------------------------------|
| integrated loudness | Average perceived loudness. Set `target.lufs` here or above |
| true peak          | Intersample peak ceiling. Set `target.true_peak` at or below |
| loudness range     | Dynamic spread. Set `target.lra` at or ABOVE this value    |

### Extended
| Field              | What it means                                              |
|--------------------|------------------------------------------------------------|
| RMS level          | Average energy in dBFS. Lower = more headroom / dynamics   |
| crest factor       | Peak minus RMS. >20 dB = very transient. <8 dB = dense     |
| dynamic range (DR) | Approximate DR score. DR14+ excellent, DR8–13 moderate     |
| DC offset          | Should be near zero. >±0.001 worth addressing before mastering |
| phase correlation  | +1 = mono-compatible, 0 = uncorrelated, negative = phase problem |
| spectral balance   | % of energy in low (<250 Hz) / mid (250–4k Hz) / high (>4k Hz) bands |
| spectral centroid  | Center of mass of spectrum in Hz. Low = bass-heavy, high = bright |

---

## Preset fields — all values

```toml
[target]
lufs      = -16.0    # integrated loudness. range: -32 to -9
true_peak = -1.0     # intersample peak ceiling dBTP. range: -3.0 to -0.1
lra       = 11.0     # loudness range LU. range: 1–20. SET AT OR ABOVE MEASURED LRA

[filters]
highpass_hz = 20.0   # remove below this Hz. 0 = disabled
lowpass_hz  = 0.0    # remove above this Hz. 0 = disabled. rarely needed

[compressor]
enabled      = false  # off by default
threshold_db = -18.0  # engage above this level
ratio        = 2.0    # 1.0 = off, 2.0 = gentle, 10.0 = limiting
attack_ms    = 20.0   # slow = preserve transients
release_ms   = 250.0  # slow = transparent
makeup_db    = 0.0    # loudnorm controls final level anyway
knee_db      = 2.0    # 0 = hard knee, 6 = very gradual

[limiter]
enabled    = true
attack_ms  = 5.0      # 0.5 = catch all peaks, 10 = preserve transient shape
release_ms = 50.0     # short = recover fast

[output]
format      = "wav"   # wav | flac | mp3 | aac
bit_depth   = 24      # 16 | 24 | 32
sample_rate = 44100   # 44100 | 48000 | 96000

[visualization]
enabled            = true
spectrogram        = true
waveform           = true
width              = 1920
spectrogram_height = 512
waveform_height    = 200
```

---

## Filter chain order (fixed)

```
highpass → lowpass → compressor → limiter → loudnorm (2-pass)
```

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

## Preset creation workflow

```sh
# 1. Measure the material
mastercraft analyze track.wav

# 2. Note: integrated loudness, loudness range, phase correlation, spectral balance

# 3. Copy a starting preset
cp presets/default.toml presets/mypreset.toml

# 4. Edit: set lufs to target, set lra >= measured lra, tune limiter
#    Edit [meta].name to match filename

# 5. Test without writing
mastercraft master track.wav -p mypreset --dry-run

# 6. Run and inspect report
mastercraft master track.wav -p mypreset
cat mastered/track/track_master_report.json | grep -E "lra|lufs|crest"
```

---

## Preset location resolution order

```
1. ./presets/<n>.toml          (working directory — use this)
2. <exe dir>/presets/<n>.toml  (bundled install)
3. ~/.config/mastercraft/presets/<n>.toml
```

---

## Add a filter stage

In `src/pipeline/process.rs`, push into `build_filter_chain()`:

```rust
// High shelf boost
filters.push("treble=g=2.0:f=8000".to_string());

// Stereo widening
filters.push("extrastereo=m=2.0".to_string());
```

Add the field to `Filters` struct in `src/config.rs`, add to preset `.toml`, rebuild.  
Full filter reference: https://ffmpeg.org/ffmpeg-filters.html
