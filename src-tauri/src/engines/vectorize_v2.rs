//! The `vectorize-v2` engine — the svg.new pipeline, ported to Rust.
//!
//! Per file: POST image → parse SSE `svg` event → `fit_to_bounds` (full-bleed
//! / +7% pad / keep) → `sync_dimensions` (~25MP default) → either write the
//! SVG directly or convert via svg.new's edit/convert for ai/dxf.

use crate::engines::proxy::{ProxyRotator, TorSession};
use crate::engines::{is_http_403, is_not_svg_parse_error, Engine, EngineError, EngineOptions, OptionDef, OptionKind};
use crate::net::http::{BoxFuture, HttpClient, PinnedClient};
use crate::svg::fit::{fit_to_bounds, sync_dimensions, FitMode};
use std::path::Path;

pub struct VectorizeV2Engine {
    client: Box<dyn HttpClient>,
    rotation: ProxyRotator,
    tor_session: TorSession,
}

impl VectorizeV2Engine {
    pub fn new() -> Self {
        VectorizeV2Engine {
            client: Box::new(PinnedClient::new().expect("failed to init pinned client")),
            rotation: ProxyRotator::new(),
            tor_session: TorSession::new(),
        }
    }

    pub fn new_with(client: Box<dyn HttpClient>) -> Self {
        VectorizeV2Engine {
            client,
            rotation: ProxyRotator::new(),
            tor_session: TorSession::new(),
        }
    }

    fn format(&self, opts: &EngineOptions) -> String {
        opts.get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("svg")
            .to_string()
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
        opts.get("suffix").and_then(|v| v.as_str()).unwrap_or("-v2").to_string()
    }

    fn skip_existing(&self, opts: &EngineOptions) -> bool {
        opts.get("skip_existing").and_then(|v| v.as_bool()).unwrap_or(false)
    }

    fn use_proxy(&self, opts: &EngineOptions) -> bool {
        opts.get("use_proxy").and_then(|v| v.as_bool()).unwrap_or(true)
    }

    fn proxy_mode(&self, opts: &EngineOptions) -> String {
        opts.get("proxy_mode").and_then(|v| v.as_str()).unwrap_or("direct").to_string()
    }

    /// Base SOCKS endpoint for Tor mode. Defaults to the local Tor daemon;
    /// overridable via the `tor_addr` option (first proxy_list line also works
    /// as a fallback for power users).
    fn tor_addr(&self, opts: &EngineOptions) -> String {
        opts.get("tor_addr")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or("socks5://127.0.0.1:9050")
            .to_string()
    }

    fn proxy_list(&self, opts: &EngineOptions) -> Vec<String> {
        opts.get("proxy_list")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|p| p.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Engine for VectorizeV2Engine {
    fn id(&self) -> &str {
        "vectorize-v2"
    }

    fn name(&self) -> &str {
        "Vectorize V2"
    }

    fn options_schema(&self) -> Vec<OptionDef> {
        vec![
            OptionDef {
                id: "format".into(),
                label: "Format".into(),
                // png/pdf dihapus: export raster/pdf dari svg.new bukan vektor
                // murni — pakai SVG/AI/DXF.
                kind: OptionKind::Select(vec![
                    ("svg".into(), "SVG".into()),
                    ("ai".into(), "AI".into()),
                    ("dxf".into(), "DXF".into()),
                ]),
                default: serde_json::json!("svg"),
            },
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
                id: "skip_existing".into(),
                label: "Skip existing".into(),
                kind: OptionKind::Bool,
                default: serde_json::json!(false),
            },
            OptionDef {
                id: "suffix".into(),
                label: "Suffix output".into(),
                kind: OptionKind::Text,
                default: serde_json::json!("-v2"),
            },
            OptionDef {
                id: "use_proxy".into(),
                label: "Pakai proxy".into(),
                kind: OptionKind::Bool,
                default: serde_json::json!(true),
            },
        ]
    }

    fn output_name(&self, file: &Path, opts: &EngineOptions) -> String {
        let ext = self.format(opts);
        let base = file.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
        format!("{base}{}.{ext}", self.suffix(opts))
    }

    fn process<'a>(
        &'a self,
        file: &'a Path,
        out_dir: &'a Path,
        opts: &'a EngineOptions,
        progress: Option<&'a crate::net::http::ProgressSink>,
    ) -> BoxFuture<'a, Result<Vec<u8>, EngineError>> {
        Box::pin(async move {
            let ext = self.format(opts);
            let base = file.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
            let out_path = out_dir.join(format!("{base}{}.{ext}", self.suffix(opts)));
            if self.skip_existing(opts) && out_path.exists() {
                return std::fs::read(&out_path).map_err(Into::into);
            }
            let img = std::fs::read(file)?;
            // Proxy list (opsi use_proxy + mode dari Settings). Proxy dipakai
            // untuk mengakali blokir per-IP svg.new (403): tiap proxy baru =
            // IP baru, rotasi otomatis saat 403/transport error.
            let mode = self.proxy_mode(opts);
            let tor = self.use_proxy(opts) && mode == "tor";
            let tor_base = self.tor_addr(opts);
            let proxies: Vec<String> = if self.use_proxy(opts) && mode == "user" {
                self.proxy_list(opts)
                    .iter()
                    .filter_map(|p| ProxyRotator::normalize(p))
                    .collect()
            } else {
                Vec::new()
            };
            // Tor mode: retry a bounded number of fresh circuits per file — a
            // single Tor exit may be blocked/quota'd, but a new circuit gives a
            // new exit IP. Beyond the cap the file fails and the batch moves on.
            const MAX_TOR_CIRCUITS: usize = 8;
            let mut tor_tries = 0usize;

            loop {
                // Reuse the live proxy across files; rotate when it dies.
                // Tidak ada cap rotasi: semua proxy dicoba, sisanya jatuh ke
                // koneksi langsung; kalau itu juga gagal → error di bawah.
                let proxy = if tor {
                    // Reuse this isolated circuit until its quota/transport fails.
                    Some(self.tor_session.proxy(&tor_base))
                } else if !proxies.is_empty() {
                    self.rotation
                        .next_live_parallel(&proxies, |p| self.client.vectorize_proxy_alive(p))
                        .await
                } else {
                    None
                };
                let svg = match self.client.vectorize(&img, proxy.as_deref(), progress).await {
                    Ok(s) => s,
                    // 403 (IP diblokir sementara) atau transport error → proxy
                    // ini mati, coba proxy berikutnya (atau direct terakhir).
                    Err(e) if tor && (is_http_403(&e) || ProxyRotator::is_transport_error(&e)) => {
                        if let Some(p) = proxy.as_deref() {
                            self.tor_session.invalidate(p);
                        }
                        tor_tries += 1;
                        if tor_tries >= MAX_TOR_CIRCUITS {
                            return Err(e);
                        }
                        continue; // new circuit next iteration
                    }
                    Err(e)
                        if proxy.is_some()
                            && (is_http_403(&e)
                                || ProxyRotator::is_transport_error(&e)
                                || is_not_svg_parse_error(&e)) =>
                    {
                        if let Some(p) = &proxy {
                            self.rotation.mark_dead(p);
                        }
                        continue;
                    }
                    Err(e) => return Err(e),
                };
                let mode = self.fit_mode(opts);
                let fitted = if mode == FitMode::None {
                    svg
                } else {
                    fit_to_bounds(&svg, mode)
                };
                let fitted = sync_dimensions(&fitted, self.target_mp(opts));
                let bytes = if ext == "svg" {
                    fitted.into_bytes()
                } else {
                    self.client.edit_convert(&fitted, &ext, proxy.as_deref()).await?
                };
                std::fs::write(&out_path, &bytes)?;
                return Ok(bytes);
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::EngineOptions;
    use crate::net::http::tests::MockHttp;

    const MOCK_SVG: &str = r##"<svg width="1200" height="896" viewBox="0 0 4800 3584"><path d="M917.61 421.33 l2966 0 l0 2005 l-2966 0 z"/></svg>"##;

    fn opts(pairs: &[(&str, serde_json::Value)]) -> EngineOptions {
        pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    fn engine() -> VectorizeV2Engine {
        VectorizeV2Engine::new_with(Box::new(MockHttp {
            svg: MOCK_SVG.to_string(),
            export: b"converted-bytes".to_vec(),
            ..MockHttp::default()
        }))
    }

    #[tokio::test]
    async fn process_produces_svg_with_normalized_origin() {
        let eng = engine();
        let dir = std::env::temp_dir().join("xix-eng-test");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("in.png");
        std::fs::write(&file, b"fake").unwrap();
        let out = eng.process(&file, &dir, &opts(&[]), None).await.unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains(r#"viewBox="0 0"#), "artboard normalized to (0,0)");
        assert!(s.contains(r#"<g transform="translate("#), "artwork shifted into origin");
        assert!(std::fs::read(dir.join("in-v2.svg")).unwrap().len() > 0, "output written");
    }

    #[tokio::test]
    async fn process_skips_existing_when_option_set() {
        let eng = engine();
        let dir = std::env::temp_dir().join("xix-eng-test2");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("in.png");
        std::fs::write(&file, b"fake").unwrap();
        std::fs::write(dir.join("in-v2.svg"), b"exists").unwrap();
        let o = opts(&[("skip_existing", serde_json::json!(true))]);
        let out = eng.process(&file, &dir, &o, None).await.unwrap();
        assert_eq!(out, b"exists");
    }

    #[tokio::test]
    async fn process_converts_to_ai_via_edit_convert() {
        let eng = engine();
        let dir = std::env::temp_dir().join("xix-eng-test3");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("in.png");
        std::fs::write(&file, b"fake").unwrap();
        let o = opts(&[("format", serde_json::json!("ai"))]);
        let out = eng.process(&file, &dir, &o, None).await.unwrap();
        assert_eq!(out, b"converted-bytes");
        assert!(dir.join("in-v2.ai").exists());
    }

    #[tokio::test]
    async fn proxy_mode_rotates_off_403_to_success() {
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mock = MockHttp {
            svg: MOCK_SVG.to_string(),
            export: b"converted-bytes".to_vec(),
            // proxy pertama & kedua diblokir 403 → harus pindah ke yang ketiga
            v2_403_proxies: vec!["http://p1:8080".into(), "http://p2:8080".into()],
            log: log.clone(),
            ..MockHttp::default()
        };
        let eng = VectorizeV2Engine::new_with(Box::new(mock));
        let dir = std::env::temp_dir().join("xix-v2-proxy");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("in.png");
        std::fs::write(&file, b"fake").unwrap();
        let o = opts(&[
            ("use_proxy", serde_json::json!(true)),
            ("proxy_mode", serde_json::json!("user")),
            (
                "proxy_list",
                serde_json::json!(["http://p1:8080", "http://p2:8080", "http://p3:8080"]),
            ),
        ]);
        let out = eng.process(&file, &dir, &o, None).await.unwrap();
        assert!(!out.is_empty());
        let calls = log.lock().unwrap().clone();
        let vecs: Vec<String> = calls
            .iter()
            .filter(|c| c.starts_with("vectorize:"))
            .cloned()
            .collect();
        assert_eq!(
            vecs,
            vec![
                "vectorize:http://p1:8080",
                "vectorize:http://p2:8080",
                "vectorize:http://p3:8080"
            ],
            "p1/p2 403 → rotate, p3 sukses: {vecs:?}"
        );
    }

    #[tokio::test]
    async fn proxy_mode_skips_dead_proxies_via_health_probe() {
        // Proxy mati (transport error) di-skip oleh health probe
        // (vectorize_proxy_alive) SEBELUM vectorize dipanggil — jadi request
        // nyata tidak pernah menggantung di proxy mati. Hanya p3 (hidup) yang
        // sampai ke vectorize.
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mock = MockHttp {
            svg: MOCK_SVG.to_string(),
            export: b"converted-bytes".to_vec(),
            // p1/p2 mati → gagal probe; p3 hidup → lolos probe & sukses
            transport_proxies: vec!["http://p1:8080".into(), "http://p2:8080".into()],
            log: log.clone(),
            ..MockHttp::default()
        };
        let eng = VectorizeV2Engine::new_with(Box::new(mock));
        let dir = std::env::temp_dir().join("xix-v2-transport");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("in.png");
        std::fs::write(&file, b"fake").unwrap();
        let o = opts(&[
            ("use_proxy", serde_json::json!(true)),
            ("proxy_mode", serde_json::json!("user")),
            (
                "proxy_list",
                serde_json::json!(["http://p1:8080", "http://p2:8080", "http://p3:8080"]),
            ),
        ]);
        let out = eng.process(&file, &dir, &o, None).await.unwrap();
        assert!(!out.is_empty());
        let calls = log.lock().unwrap().clone();
        // Probe menolak p1/p2 sebelum vectorize; hanya p3 yang di-vectorize.
        let vecs: Vec<String> = calls
            .iter()
            .filter(|c| c.starts_with("vectorize:"))
            .cloned()
            .collect();
        assert_eq!(
            vecs,
            vec!["vectorize:http://p3:8080"],
            "p1/p2 mati di-skip probe, hanya p3 di-vectorize: {vecs:?}"
        );
        // Health probe dijalankan untuk p1/p2 (yang mati) lalu p3.
        assert!(
            calls.contains(&"v2_alive:http://p1:8080".to_string())
                && calls.contains(&"v2_alive:http://p2:8080".to_string())
                && calls.contains(&"v2_alive:http://p3:8080".to_string()),
            "semua kandidat harus di-probe: {calls:?}"
        );
    }

    #[tokio::test]
    async fn all_proxies_transport_dead_falls_back_to_direct() {
        // Semua proxy mati (transport error) → fallback ke koneksi langsung
        // yang sukses.
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mock = MockHttp {
            svg: MOCK_SVG.to_string(),
            transport_proxies: vec!["http://p1:8080".into()],
            log: log.clone(),
            ..MockHttp::default()
        };
        let eng = VectorizeV2Engine::new_with(Box::new(mock));
        let dir = std::env::temp_dir().join("xix-v2-transport-direct");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("in.png");
        std::fs::write(&file, b"fake").unwrap();
        let o = opts(&[
            ("use_proxy", serde_json::json!(true)),
            ("proxy_mode", serde_json::json!("user")),
            ("proxy_list", serde_json::json!(["http://p1:8080"])),
        ]);
        eng.process(&file, &dir, &o, None).await.unwrap();
        let calls = log.lock().unwrap().clone();
        assert!(
            calls.iter().any(|c| c == "vectorize:direct"),
            "harus fallback ke direct: {calls:?}"
        );
    }

    #[tokio::test]
    async fn use_proxy_false_goes_direct() {
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mock = MockHttp {
            svg: MOCK_SVG.to_string(),
            log: log.clone(),
            ..MockHttp::default()
        };
        let eng = VectorizeV2Engine::new_with(Box::new(mock));
        let dir = std::env::temp_dir().join("xix-v2-noproxy");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("in.png");
        std::fs::write(&file, b"fake").unwrap();
        let o = opts(&[
            ("use_proxy", serde_json::json!(false)),
            ("proxy_mode", serde_json::json!("user")),
            ("proxy_list", serde_json::json!(["http://p1:8080"])),
        ]);
        eng.process(&file, &dir, &o, None).await.unwrap();
        let calls = log.lock().unwrap().clone();
        assert!(
            calls.iter().any(|c| c == "vectorize:direct"),
            "use_proxy=false harus lewat direct: {calls:?}"
        );
    }

    #[tokio::test]
    async fn process_propagates_network_failure() {
        let mut mock = MockHttp::default();
        mock.fail = true;
        let eng = VectorizeV2Engine::new_with(Box::new(mock));
        let dir = std::env::temp_dir().join("xix-eng-test4");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("in.png");
        std::fs::write(&file, b"fake").unwrap();
        let err = eng.process(&file, &dir, &opts(&[]), None).await.unwrap_err();
        assert!(matches!(err, EngineError::Other(_)));
    }

    #[tokio::test]
    async fn tor_mode_reuses_session_across_successful_files() {
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mock = MockHttp {
            svg: MOCK_SVG.to_string(),
            log: log.clone(),
            ..MockHttp::default()
        };
        let eng = VectorizeV2Engine::new_with(Box::new(mock));
        let dir = std::env::temp_dir().join("xix-v2-tor-sticky");
        std::fs::create_dir_all(&dir).unwrap();
        let first = dir.join("a.png");
        let second = dir.join("b.png");
        std::fs::write(&first, b"fake").unwrap();
        std::fs::write(&second, b"fake").unwrap();
        let o = opts(&[("proxy_mode", serde_json::json!("tor"))]);

        eng.process(&first, &dir, &o, None).await.unwrap();
        eng.process(&second, &dir, &o, None).await.unwrap();

        let calls = log.lock().unwrap().clone();
        let proxies: Vec<&str> = calls
            .iter()
            .filter_map(|c| c.strip_prefix("vectorize:"))
            .collect();
        assert_eq!(proxies.len(), 2, "one successful request per file: {calls:?}");
        assert_eq!(
            proxies[0], proxies[1],
            "successful files must reuse one Tor circuit until 403/transport failure"
        );
    }
}
