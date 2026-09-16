//! The PngToSvg engine — offline image-to-SVG, the highest-quality vector
//! engine in XIX-Vectorizer. It runs under a bundled portable Node so the
//! app stays self-contained and needs no network at all.
//!
//! The conversion core lives in `pngtosvg-runtime/worker.js` (pure JS, no
//! DOM). A small runner decodes the image (pngjs/jpeg-js in
//! `pngtosvg-runtime/lib`) and feeds the core's `Qn(image, settings)` entry
//! point. The engine shells out to the bundled `node.exe` (same pattern as
//! the SVG Converter → Inkscape):
//!
//!   `node.exe runner.js <input> <raw.svg> <max-dim> <colors>`
//!
//! then reads the raw SVG and applies the usual `ensure_viewbox` → fit
//! (full-bleed / +7% pad / keep) → `sync_dimensions` (~25MP default) so the
//! output matches every other engine. Fully offline: no rate limit, no
//! token, no proxy, no watermark.

use crate::engines::{Engine, EngineError, EngineOptions, OptionDef, OptionKind};
use crate::net::http::BoxFuture;
use crate::svg::fit::{ensure_viewbox, fit_to_bounds, sync_dimensions, FitMode};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

/// Bundled portable Node (set once at app startup via [`set_bundled_node`]
/// from the resource dir). `None`/unset → fall back to a system install.
static BUNDLED_NODE: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Point the engine at the bundled portable Node executable (or `None` when
/// the bundle is missing, e.g. dev without running `fetch-node.bat`).
pub fn set_bundled_node(exe: Option<PathBuf>) {
    let _ = BUNDLED_NODE.set(exe);
}

fn bundled_node() -> Option<String> {
    BUNDLED_NODE
        .get()
        .and_then(|o| o.as_ref())
        .filter(|p| p.exists())
        .map(|p| p.to_string_lossy().into_owned())
}

/// Working directory for the runner (contains runner.js, worker.js, lib/).
fn runtime_dir() -> Option<PathBuf> {
    let candidates = [
        // bundled resource dir (release)
        PathBuf::from("pngtosvg-runtime"),
        // repo dev layouts
        PathBuf::from("src-tauri/pngtosvg-runtime"),
        PathBuf::from("desktop/src-tauri/pngtosvg-runtime"),
    ];
    candidates.into_iter().find(|p| p.join("runner.js").exists())
}

pub struct PngToSvgEngine {
    node: Mutex<Option<String>>,
}

impl PngToSvgEngine {
    pub fn new() -> Self {
        PngToSvgEngine { node: Mutex::new(None) }
    }

    /// Test hook: pin a specific node executable.
    pub fn new_with(node: impl Into<String>) -> Self {
        PngToSvgEngine { node: Mutex::new(Some(node.into())) }
    }

    fn fit_mode(&self, opts: &EngineOptions) -> FitMode {
        match opts.get("fit").and_then(|v| v.as_str()) {
            Some("pad") => FitMode::Pad,
            Some("none") => FitMode::None,
            _ => FitMode::Fit,
        }
    }

    fn target_mp(&self, opts: &EngineOptions) -> f64 {
        opts.get("target_mp").and_then(|v| v.as_f64()).unwrap_or(25.0)
    }

    fn suffix(&self, opts: &EngineOptions) -> String {
        opts.get("suffix").and_then(|v| v.as_str()).unwrap_or("-v3").to_string()
    }

    fn skip_existing(&self, opts: &EngineOptions) -> bool {
        opts.get("skip_existing").and_then(|v| v.as_bool()).unwrap_or(false)
    }

    /// Downscale cap for the worker input — locked at 2000px (longest side,
    /// aspect preserved). Removed from the options schema: users don't need to
    /// tune it, 2000 is the sweet spot between trace detail and speed.
    fn max_dim(&self, _opts: &EngineOptions) -> u32 {
        2000
    }

    fn colors(&self, opts: &EngineOptions) -> u32 {
        opts.get("colors").and_then(|v| v.as_f64()).map(|c| c as u32).unwrap_or(0)
    }

    /// Locate node (bundled portable first, then PATH) and cache it.
    fn get_node(&self) -> Result<String, EngineError> {
        {
            let cached = self.node.lock().unwrap();
            if let Some(p) = cached.as_ref() {
                return Ok(p.clone());
            }
        }
        let found = bundled_node().or_else(|| {
            if Command::new("node").arg("--version").output().map(|o| o.status.success()).unwrap_or(false) {
                return Some("node".into());
            }
            None
        });
        let path = found.ok_or_else(|| {
            EngineError::Other(
                "Node tidak ditemukan (bundled ataupun terinstall). Jalankan tools/fetch-node.bat.".into(),
            )
        })?;
        *self.node.lock().unwrap() = Some(path.clone());
        Ok(path)
    }
}

impl Engine for PngToSvgEngine {
    fn id(&self) -> &str {
        "pngtosvg"
    }

    fn name(&self) -> &str {
        "Vectorize V3"
    }

    fn options_schema(&self) -> Vec<OptionDef> {
        vec![
            OptionDef {
                id: "fit".into(),
                label: "Fit".into(),
                kind: OptionKind::Select(vec![
                    ("fit".into(), "Full-bleed".into()),
                    ("pad".into(), "+7% padding".into()),
                    ("none".into(), "Original".into()),
                ]),
                default: serde_json::json!("fit"),
            },
            OptionDef {
                id: "target_mp".into(),
                label: "Target MP".into(),
                kind: OptionKind::Number { min: 15.0, max: 65.0, step: 1.0 },
                default: serde_json::json!(25),
            },
            OptionDef {
                id: "colors".into(),
                label: "Color (0 = Auto)".into(),
                kind: OptionKind::Number { min: 0.0, max: 32.0, step: 1.0 },
                default: serde_json::json!(0),
            },
            OptionDef {
                id: "skip_existing".into(),
                label: "Skip existing".into(),
                kind: OptionKind::Bool,
                default: serde_json::json!(false),
            },
            OptionDef {
                id: "suffix".into(),
                label: "Suffix output".into(),
                kind: OptionKind::Text,
                default: serde_json::json!("-v3"),
            },
        ]
    }

    fn output_name(&self, file: &Path, opts: &EngineOptions) -> String {
        let base = file.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
        format!("{base}{}.svg", self.suffix(opts))
    }

    fn process<'a>(
        &'a self,
        file: &'a Path,
        out_dir: &'a Path,
        opts: &'a EngineOptions,
        _progress: Option<&'a crate::net::http::ProgressSink>,
    ) -> BoxFuture<'a, Result<Vec<u8>, EngineError>> {
        Box::pin(async move {
            let base = file.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
            let out_path = out_dir.join(format!("{base}{}.svg", self.suffix(opts)));
            if self.skip_existing(opts) && out_path.exists() {
                return std::fs::read(&out_path).map_err(Into::into);
            }
            let node = self.get_node()?;
            let runtime = runtime_dir()
                .ok_or_else(|| EngineError::Other("pngtosvg-runtime tidak ditemukan".into()))?;
            let runner = runtime.join("runner.js");
            if !runner.exists() {
                return Err(EngineError::Other(format!("runner.js tidak ada: {runner:?}")));
            }

            // Runner writes the raw SVG to a temp path next to the output.
            let raw_path = out_dir.join(format!(".{base}.raw.svg"));
            let out = Command::new(&node)
                .arg(&runner)
                .arg(file)
                .arg(&raw_path)
                .arg(self.max_dim(opts).to_string())
                .arg(self.colors(opts).to_string())
                .output()
                .map_err(|e| EngineError::Other(format!("node exec: {e}")))?;
            if !out.status.success() {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let snippet: String = stderr.chars().take(200).collect();
                let _ = std::fs::remove_file(&raw_path);
                return Err(EngineError::Other(format!("runner gagal: {snippet}")));
            }
            let raw = std::fs::read(&raw_path).map_err(|e| {
                let _ = std::fs::remove_file(&raw_path);
                EngineError::Io(e)
            })?;
            let _ = std::fs::remove_file(&raw_path);

            let svg = String::from_utf8(raw).map_err(|e| EngineError::Parse(e.to_string()))?;
            let svg = ensure_viewbox(&svg);
            let mode = self.fit_mode(opts);
            let fitted = if mode == FitMode::None {
                svg
            } else {
                fit_to_bounds(&svg, mode)
            };
            let fitted = sync_dimensions(&fitted, self.target_mp(opts));
            std::fs::write(&out_path, fitted.as_bytes())?;
            Ok(fitted.into_bytes())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::EngineOptions;

    fn opts(pairs: &[(&str, serde_json::Value)]) -> EngineOptions {
        pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    #[test]
    fn output_name_matches_suffix() {
        let eng = PngToSvgEngine::new_with("node");
        assert_eq!(eng.output_name(Path::new("a.png"), &opts(&[])), "a-v3.svg");
        let o = opts(&[("suffix", serde_json::json!("-x"))]);
        assert_eq!(eng.output_name(Path::new("a.png"), &o), "a-x.svg");
    }

    #[tokio::test]
    async fn process_errors_cleanly_when_node_missing() {
        let eng = PngToSvgEngine::new_with("/nonexistent/node.exe");
        let dir = std::env::temp_dir().join("xix-pngtosvg-missing");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.png");
        std::fs::write(&file, b"fake").unwrap();
        let err = eng.process(&file, &dir, &opts(&[]), None).await.unwrap_err();
        assert!(matches!(err, EngineError::Other(_)), "got: {err}");
    }

    #[test]
    fn runtime_dir_found_in_dev_layout() {
        assert!(runtime_dir().is_some(), "pngtosvg-runtime harus ada di repo");
    }
}
