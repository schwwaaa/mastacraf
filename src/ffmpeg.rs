use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::process::Command;

/// Null device path — platform-specific
#[cfg(target_family = "windows")]
pub const NULL_DEV: &str = "NUL";
#[cfg(not(target_family = "windows"))]
pub const NULL_DEV: &str = "/dev/null";

// ── Binary wrapper ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Ffmpeg {
    pub path: PathBuf,
}

pub struct FfmpegOutput {
    pub success: bool,
    pub stdout:  String,
    pub stderr:  String,
}

impl Ffmpeg {
    /// Locate ffmpeg: bundled next to the executable first, then $PATH.
    pub fn find() -> Result<Self> {
        // 1. Bundled alongside executable
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                #[cfg(target_family = "windows")]
                let bundled = dir.join("ffmpeg.exe");
                #[cfg(not(target_family = "windows"))]
                let bundled = dir.join("ffmpeg");

                if bundled.exists() {
                    return Ok(Ffmpeg { path: bundled });
                }
            }
        }

        // 2. $PATH lookup
        if let Ok(found) = which::which("ffmpeg") {
            return Ok(Ffmpeg { path: found });
        }

        bail!(
            "ffmpeg not found.\n\
             \n\
             Install it with your package manager:\n\
             \n\
             macOS:   brew install ffmpeg\n\
             Ubuntu:  sudo apt install ffmpeg\n\
             Arch:    sudo pacman -S ffmpeg\n\
             Windows: https://ffmpeg.org/download.html\n\
             \n\
             Or place the ffmpeg binary next to the mastacraf executable."
        )
    }

    /// Run ffmpeg with the given arguments, returning stdout + stderr.
    pub fn run(&self, args: &[&str]) -> Result<FfmpegOutput> {
        let out = Command::new(&self.path)
            .args(args)
            .output()
            .with_context(|| format!("Failed to spawn ffmpeg at {}", self.path.display()))?;

        Ok(FfmpegOutput {
            success: out.status.success(),
            stdout:  String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr:  String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }

    /// Run and bail on non-zero exit.
    pub fn run_ok(&self, args: &[&str]) -> Result<FfmpegOutput> {
        let out = self.run(args)?;
        if !out.success {
            bail!("ffmpeg exited with an error:\n{}", out.stderr.trim());
        }
        Ok(out)
    }

    /// Print the ffmpeg command before running (verbose mode).
    pub fn run_verbose(&self, args: &[&str], verbose: bool) -> Result<FfmpegOutput> {
        if verbose {
            use colored::Colorize;
            let cmd = std::iter::once(self.path.display().to_string())
                .chain(args.iter().map(|s| {
                    if s.contains(',') || s.contains(' ') {
                        format!("\"{}\"", s)
                    } else {
                        s.to_string()
                    }
                }))
                .collect::<Vec<_>>()
                .join(" ");
            println!("  {} {}", "→".dimmed(), cmd.dimmed());
        }
        self.run_ok(args)
    }
}
