//! The `vectorize-v1` engine — the svgai.org pipeline, ported to Rust.
//!
//! svgai is SYNC (one multipart POST returns the SVG) but enforces a ~3-5
//! free conversions/day quota per client IP (HTTP 402). To work around it the
//! engine rotates user-supplied proxies (modal in the UI): a proxy is
//! health-checked against the cheap paywall endpoint, reused while it works,
//! blacklisted on 402 / transport errors, and 429 backs off. When every proxy
//! is dead it falls back to the direct IP (free quota only).
//!
//! Proxy formats (one per line): `socks5://ip:port`, `http://ip:port`,
//! `http://user:pass@ip:port`. Like the CLI, TLS certs are accepted as-is on
//! the svgai client because anon proxies commonly MITM (payload is public).

use crate::engines::proxy::{ProxyRotator, TorSession};
use crate::engines::{is_not_svg_parse_error, Engine, EngineError, EngineOptions, OptionDef, OptionKind};
use crate::net::http::{BoxFuture, HttpClient, PinnedClient};
use crate::svg::fit::{fit_to_bounds, sync_dimensions, FitMode};
use std::path::Path;
use std::time::Duration;

const MAX_RATELIMIT_RETRIES: usize = 3;

pub struct VectorizeV1Engine {
    client: Box<dyn HttpClient>,
    rotation: ProxyRotator,
    tor_session: TorSession,
}

impl VectorizeV1Engine {
    pub fn new() -> Self {
        VectorizeV1Engine {
            client: Box::new(PinnedClient::new().expect("failed to init client")),
            rotation: ProxyRotator::new(),
            tor_session: TorSession::new(),
        }
    }

    pub fn new_with(client: Box<dyn HttpClient>) -> Self {
        VectorizeV1Engine {
            client,
            rotation: ProxyRotator::new(),
            tor_session: TorSession::new(),
        }
    }

    fn fit_mode(&self, opts: &EngineOptions) -> FitMode {
        match opts.get("fit").and_then(|v| v.as_str()) {
            Some("pad") => FitMode::Pad,
            Some("none") => FitMode::None,
            _ => FitMode::Fit, // v1 default = full-bleed
        }
    }

    fn target_mp(&self, opts: &EngineOptions) -> f64 {
        opts.get("target_mp").and_then(|v| v.as_f64()).unwrap_or(25.0)
    }

    fn suffix(&self, opts: &EngineOptions) -> String {
        opts.get("suffix").and_then(|v| v.as_str()).unwrap_or("-v1").to_string()
    }

    fn skip_existing(&self, opts: &EngineOptions) -> bool {
        opts.get("skip_existing").and_then(|v| v.as_bool()).unwrap_or(false)
    }

    fn proxy_mode(&self, opts: &EngineOptions) -> String {
        opts.get("proxy_mode").and_then(|v| v.as_str()).unwrap_or("direct").to_string()
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

    /// Base SOCKS endpoint for Tor mode (defaults to local Tor daemon).
    fn tor_addr(&self, opts: &EngineOptions) -> String {
        opts.get("tor_addr")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or("socks5://127.0.0.1:9050")
            .to_string()
    }

    fn mime_for(name: &str) -> &'static str {
        let ext = Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "png" => "image/png",
            "webp" => "image/webp",
            _ => "image/jpeg",
        }
    }

}

impl Engine for VectorizeV1Engine {
    fn id(&self) -> &str {
        "vectorize-v1"
    }

    fn name(&self) -> &str {
        "Vectorize V1"
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
                id: "skip_existing".into(),
                label: "Skip existing".into(),
                kind: OptionKind::Bool,
                default: serde_json::json!(false),
            },
            OptionDef {
                id: "suffix".into(),
                label: "Suffix output".into(),
                kind: OptionKind::Text,
                default: serde_json::json!("-v1"),
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
            let img = std::fs::read(file)?;
            let filename = file.file_name().and_then(|s| s.to_str()).unwrap_or("image").to_string();
            let mime = Self::mime_for(&filename);

            let mode = self.proxy_mode(opts);
            let tor = mode == "tor";
            let tor_base = self.tor_addr(opts);
            let proxies: Vec<String> = if mode == "user" {
                self.proxy_list(opts)
                    .iter()
                    .filter_map(|p| ProxyRotator::normalize(p))
                    .collect()
            } else {
                Vec::new()
            };
            // Tor mode: each 402/transport failure means this exit IP is spent
            // or blocked — spin a fresh circuit (new exit IP) up to a cap.
            const MAX_TOR_CIRCUITS: usize = 8;
            let mut tor_tries = 0usize;

            let mut retries = 0usize;
            loop {
                // Ensure a live proxy (reused across files until it fails),
                // health-checked against the cheap svgai paywall endpoint.
                let proxy = if tor {
                    Some(self.tor_session.proxy(&tor_base))
                } else if !proxies.is_empty() {
                    self.rotation
                        .next_live(&proxies, |p| self.client.svgai_proxy_alive(p))
                        .await
                } else {
                    None
                };

                match self
                    .client
                    .svgai_convert(&img, &mime, &filename, proxy.as_deref())
                    .await
                {
                    Ok(bytes) => {
                        let raw = String::from_utf8_lossy(&bytes).into_owned();
                        let mode = self.fit_mode(opts);
                        let fitted = if mode == FitMode::None {
                            raw
                        } else {
                            fit_to_bounds(&raw, mode)
                        };
                        let fitted = sync_dimensions(&fitted, self.target_mp(opts));
                        let out = fitted.into_bytes();
                        std::fs::write(&out_path, &out)?;
                        return Ok(out);
                    }
                    Err(EngineError::Quota(_)) if tor => {
                        if let Some(p) = proxy.as_deref() {
                            self.tor_session.invalidate(p);
                        }
                        // Exit IP kehabisan kuota harian → sirkuit baru = IP baru.
                        tor_tries += 1;
                        if tor_tries >= MAX_TOR_CIRCUITS {
                            return Err(EngineError::Quota(
                                "quota exhausted: semua sirkuit Tor yang dicoba kena 402 — coba lagi nanti".into(),
                            ));
                        }
                        continue;
                    }
                    Err(EngineError::Quota(_)) => {
                        // Proxy kena 402 → blacklist & rotate. Tidak ada cap
                        // retry: loop berakhir natural — begitu semua proxy
                        // habis, next_live mengembalikan None → fallback ke
                        // koneksi langsung; kalau itu juga 402 → error di bawah.
                        if proxy.is_some() {
                            if let Some(p) = &proxy {
                                self.rotation.mark_dead(p);
                            }
                            continue;
                        }
                        // proxy None = koneksi langsung (tanpa proxy) kena 402.
                        if proxies.is_empty() {
                            return Err(EngineError::Quota(
                                "quota exhausted: kuota harian svgai untuk IP ini habis (402) — tunggu reset kuota atau tambah proxy".into(),
                            ));
                        }
                        return Err(EngineError::Quota(
                            "quota exhausted, no proxy left: semua proxy kena 402 dan koneksi langsung juga habis — isi ulang list proxy".into(),
                        ));
                    }
                    Err(EngineError::RateLimit) => {
                        if retries < MAX_RATELIMIT_RETRIES {
                            retries += 1;
                            tokio::time::sleep(Duration::from_secs(15 * retries as u64)).await;
                            continue;
                        }
                        return Err(EngineError::RateLimit);
                    }
                    Err(e) if tor && ProxyRotator::is_transport_error(&e) => {
                        if let Some(p) = proxy.as_deref() {
                            self.tor_session.invalidate(p);
                        }
                        // Sirkuit Tor ini mati/lambat → sirkuit baru = exit baru.
                        tor_tries += 1;
                        if tor_tries >= MAX_TOR_CIRCUITS {
                            return Err(e);
                        }
                        continue;
                    }
                    Err(e)
                        if proxy.is_some()
                            && (ProxyRotator::is_transport_error(&e) || is_not_svg_parse_error(&e)) =>
                    {
                        // Proxy mati ATAU menyisipkan halaman non-SVG (block
                        // page free proxy dengan status 200) → blacklist &
                        // rotate; sisanya jatuh ke koneksi langsung.
                        if let Some(p) = &proxy {
                            self.rotation.mark_dead(p);
                        }
                        continue;
                    }
                    Err(e) => return Err(e),
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::EngineOptions;
    use crate::net::http::tests::MockHttp;
    use std::sync::{Arc, Mutex};

    const MOCK_SVG: &str = r##"<svg width="1200" height="896" viewBox="0 0 4800 3584"><path d="M917.61 421.33 l2966 0 l0 2005 l-2966 0 z"/></svg>"##;

    fn opts(pairs: &[(&str, serde_json::Value)]) -> EngineOptions {
        pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    fn tempdir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("xix-v1-{name}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn direct_mode_converts_and_normalizes_origin() {
        let mock = MockHttp {
            v1_svg: MOCK_SVG.as_bytes().to_vec(),
            ..MockHttp::default()
        };
        let eng = VectorizeV1Engine::new_with(Box::new(mock));
        let dir = tempdir("direct");
        let file = dir.join("in.png");
        std::fs::write(&file, b"fake").unwrap();
        let out = eng.process(&file, &dir, &opts(&[]), None).await.unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains(r#"viewBox="0 0"#));
        assert!(s.contains(r#"<g transform="translate("#));
        assert!(dir.join("in-v1.svg").exists());
    }

    #[tokio::test]
    async fn user_mode_rotates_off_quota_proxies_to_success() {
        let mock = MockHttp {
            v1_svg: MOCK_SVG.as_bytes().to_vec(),
            v1_quota: vec!["socks5://p1:1080".into(), "http://p2:8080".into()],
            ..MockHttp::default()
        };
        let eng = VectorizeV1Engine::new_with(Box::new(mock));
        let dir = tempdir("rotate");
        let file = dir.join("in.png");
        std::fs::write(&file, b"fake").unwrap();
        let o = opts(&[
            ("proxy_mode", serde_json::json!("user")),
            (
                "proxy_list",
                serde_json::json!(["socks5://p1:1080", "http://p2:8080", "socks5://p3:1080"]),
            ),
        ]);
        let out = eng.process(&file, &dir, &o, None).await.unwrap();
        assert!(!out.is_empty());
        // p1 quota → p2 quota → p3 ok (health-checks precede each convert)
        assert!(dir.join("in-v1.svg").exists());
    }

    #[tokio::test]
    async fn all_proxies_dead_falls_back_to_direct() {
        let mock = MockHttp {
            v1_svg: MOCK_SVG.as_bytes().to_vec(),
            v1_quota: vec!["socks5://p1:1080".into()],
            ..MockHttp::default()
        };
        let eng = VectorizeV1Engine::new_with(Box::new(mock));
        let dir = tempdir("falldirect");
        let file = dir.join("in.png");
        std::fs::write(&file, b"fake").unwrap();
        let o = opts(&[
            ("proxy_mode", serde_json::json!("user")),
            ("proxy_list", serde_json::json!(["socks5://p1:1080"])),
        ]);
        let out = eng.process(&file, &dir, &o, None).await.unwrap();
        assert!(!out.is_empty());
    }

    #[tokio::test]
    async fn quota_rotation_tries_all_proxies_beyond_old_cap() {
        // Regresi: MAX_QUOTA_RETRIES lama (20) membuat engine menyerah
        // "no proxy left" padahal masih ada proxy sehat di posisi > 20.
        // Rotasi sekarang harus mencoba SEMUA proxy sebelum menyerah.
        let quota: Vec<String> = (0..24).map(|i| format!("socks5://p{i}:1080")).collect();
        let list: Vec<serde_json::Value> = (0..25)
            .map(|i| serde_json::json!(format!("socks5://p{i}:1080")))
            .collect();
        let mock = MockHttp {
            v1_svg: MOCK_SVG.as_bytes().to_vec(),
            v1_quota: quota,
            ..MockHttp::default()
        };
        let eng = VectorizeV1Engine::new_with(Box::new(mock));
        let dir = tempdir("many-proxies");
        let file = dir.join("in.png");
        std::fs::write(&file, b"fake").unwrap();
        let o = opts(&[
            ("proxy_mode", serde_json::json!("user")),
            ("proxy_list", serde_json::Value::Array(list)),
        ]);
        let out = eng.process(&file, &dir, &o, None).await.unwrap();
        assert!(!out.is_empty());
        assert!(dir.join("in-v1.svg").exists());
    }

    #[tokio::test]
    async fn all_proxies_and_direct_quota_errors_with_no_proxy_left() {
        // Semua proxy 402 + direct 402 → error "no proxy left" (bukan
        // retry tak berujung ke direct).
        let mock = MockHttp {
            v1_svg: Vec::new(),
            v1_quota: vec![
                "socks5://p1:1080".into(),
                "socks5://p2:1080".into(),
                "direct".into(), // koneksi langsung juga 402
            ],
            ..MockHttp::default()
        };
        let eng = VectorizeV1Engine::new_with(Box::new(mock));
        let dir = tempdir("all-dead");
        let file = dir.join("in.png");
        std::fs::write(&file, b"fake").unwrap();
        let o = opts(&[
            ("proxy_mode", serde_json::json!("user")),
            (
                "proxy_list",
                serde_json::json!(["socks5://p1:1080", "socks5://p2:1080"]),
            ),
        ]);
        let err = eng.process(&file, &dir, &o, None).await.unwrap_err();
        assert!(err.to_string().contains("no proxy left"), "{err}");
    }

    #[tokio::test]
    async fn direct_mode_quota_errors_fast_not_after_retries() {
        // Mode direct (tanpa proxy), 402 → error langsung, bukan 20×
        // percobaan ke direct yang pasti gagal.
        let log = Arc::new(Mutex::new(Vec::new()));
        let mock = MockHttp {
            v1_quota: vec!["direct".into()],
            log: log.clone(),
            ..MockHttp::default()
        };
        let eng = VectorizeV1Engine::new_with(Box::new(mock));
        let dir = tempdir("direct-quota");
        let file = dir.join("in.png");
        std::fs::write(&file, b"fake").unwrap();
        let err = eng.process(&file, &dir, &opts(&[]), None).await.unwrap_err();
        assert!(err.to_string().contains("quota exhausted"), "{err}");
        let calls = log.lock().unwrap().clone();
        let converts = calls
            .iter()
            .filter(|c| c.starts_with("svgai_convert:"))
            .count();
        assert_eq!(converts, 1, "direct 402 harus gagal cepat, bukan retry 20×: {calls:?}");
    }

    #[tokio::test]
    async fn dead_proxy_blacklisted_after_quota_and_live_reused() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mock = MockHttp {
            v1_svg: MOCK_SVG.as_bytes().to_vec(),
            v1_quota: vec!["socks5://p1:1080".into(), "socks5://p2:1080".into()],
            log: log.clone(),
            ..MockHttp::default()
        };
        let eng = VectorizeV1Engine::new_with(Box::new(mock));
        let dir = tempdir("blacklist");
        let file = dir.join("in.png");
        std::fs::write(&file, b"fake").unwrap();
        let o = opts(&[
            ("proxy_mode", serde_json::json!("user")),
            (
                "proxy_list",
                serde_json::json!(["socks5://p1:1080", "socks5://p2:1080", "socks5://p3:1080"]),
            ),
        ]);
        eng.process(&file, &dir, &o, None).await.unwrap();
        eng.process(&file, &dir, &o, None).await.unwrap(); // second file
        let calls = log.lock().unwrap().clone();
        let alive_calls = calls.iter().filter(|c| c.starts_with("alive:")).count();
        // File 1: alive p1, alive p2, alive p3. File 2 reuses p3 (no re-test).
        assert_eq!(alive_calls, 3, "blacklisted p1/p2 must not be re-tested: {calls:?}");
        let converts = calls.iter().filter(|c| c.starts_with("svgai_convert:")).count();
        assert_eq!(converts, 4, "p1,p2 quota + p3 ok + p3 reused: {calls:?}");
    }

    #[tokio::test]
    async fn tor_mode_reuses_session_across_successful_files() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mock = MockHttp {
            v1_svg: MOCK_SVG.as_bytes().to_vec(),
            log: log.clone(),
            ..MockHttp::default()
        };
        let eng = VectorizeV1Engine::new_with(Box::new(mock));
        let dir = tempdir("tor-sticky");
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
            .filter_map(|c| c.strip_prefix("svgai_convert:"))
            .collect();
        assert_eq!(proxies.len(), 2, "one successful request per file: {calls:?}");
        assert_eq!(
            proxies[0], proxies[1],
            "successful files must reuse one Tor circuit until quota/transport failure"
        );
    }
    #[tokio::test]
    async fn not_svg_proxy_blacklisted_and_file_rotates_to_success() {
        // Free proxy menyisipkan block page (HTTP 200, bukan SVG) → proxy
        // di-blacklist dan file dirotasi ke proxy berikutnya, bukan gagal.
        let log = Arc::new(Mutex::new(Vec::new()));
        let mock = MockHttp {
            v1_svg: MOCK_SVG.as_bytes().to_vec(),
            v1_not_svg: vec!["socks5://p1:1080".into()],
            log: log.clone(),
            ..MockHttp::default()
        };
        let eng = VectorizeV1Engine::new_with(Box::new(mock));
        let dir = tempdir("not-svg");
        let file = dir.join("in.png");
        std::fs::write(&file, b"fake").unwrap();
        let o = opts(&[
            ("proxy_mode", serde_json::json!("user")),
            (
                "proxy_list",
                serde_json::json!(["socks5://p1:1080", "http://p2:8080"]),
            ),
        ]);
        let out = eng.process(&file, &dir, &o, None).await.unwrap();
        assert!(!out.is_empty(), "file harus sukses lewat p2");
        assert!(dir.join("in-v1.svg").exists());
        let calls = log.lock().unwrap().clone();
        let converts: Vec<&str> = calls
            .iter()
            .filter_map(|c| c.strip_prefix("svgai_convert:"))
            .collect();
        assert_eq!(
            converts,
            vec!["socks5://p1:1080", "http://p2:8080"],
            "p1 non-SVG → blacklist → p2 sukses: {calls:?}"
        );
    }
}

