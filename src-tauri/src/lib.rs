pub mod batch;
pub mod config;
pub mod engines;
pub mod img;
pub mod licensing;
pub mod net;
pub mod secure;
pub mod svg;
pub mod tor;

use crate::batch::{run_batch_with_trial_gate, TrialStartGate};
use crate::config::{load as config_load, save as config_save, AppConfig};
use crate::engines::{common_batch_options, EngineOptions, OptionDef};
use crate::licensing::{AccessDecision, LicenseState, LicenseStatus, LicensingState};
use crate::net::freeproxy::{self, Candidate};
use crate::tor::{resolve_runtime, TorManager, TorRuntimePaths};
use parking_lot::Mutex;
use serde::Serialize;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Serialize)]
struct EngineInfo {
    id: String,
    name: String,
    options_schema: Vec<OptionDef>,
    input_exts: Vec<String>,
}

#[derive(Serialize)]
struct FileInfo {
    path: String,
    name: String,
    size: u64,
    ext: String,
}

/// Shared batch control. Every start gets fresh controls before any Tor
/// bootstrap work, so Stop can cancel that exact generation without a later
/// reset erasing the request.
#[derive(Default)]
struct BatchState {
    next_generation: AtomicU64,
    active: Mutex<Option<BatchStart>>,
}

#[derive(Clone)]
struct BatchStart {
    generation: u64,
    cancel: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
}

impl BatchState {
    fn begin_start(&self) -> BatchStart {
        let start = BatchStart {
            generation: self.next_generation.fetch_add(1, Ordering::Relaxed) + 1,
            cancel: Arc::new(AtomicBool::new(false)),
            pause: Arc::new(AtomicBool::new(false)),
        };
        *self.active.lock() = Some(start.clone());
        start
    }

    fn cancel_active(&self) {
        if let Some(start) = self.active.lock().as_ref() {
            start.cancel.store(true, Ordering::Release);
        }
    }

    fn pause_active(&self, paused: bool) {
        if let Some(start) = self.active.lock().as_ref() {
            start.pause.store(paused, Ordering::Release);
        }
    }
}

impl BatchStart {
    fn into_controls(
        self,
        state: &BatchState,
    ) -> Result<(Arc<AtomicBool>, Arc<AtomicBool>), String> {
        let active_generation = state.active.lock().as_ref().map(|active| active.generation);
        if active_generation != Some(self.generation) {
            return Err("batch start was superseded".into());
        }
        if self.cancel.load(Ordering::Acquire) {
            return Err("batch cancelled before processing started".into());
        }
        Ok((self.cancel, self.pause))
    }
}

#[derive(Clone)]
struct TorResources {
    paths: Result<TorRuntimePaths, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct TorStatus {
    state: &'static str,
    message: String,
}

fn prepare_tor_inputs<T>(
    paths: Result<T, String>,
    app_data_dir: Result<PathBuf, String>,
) -> Result<(T, PathBuf), String> {
    let paths = paths?;
    let data_dir = app_data_dir?.join("tor-data");
    Ok((paths, data_dir))
}

fn finish_tor_preparation<T>(
    result: Result<T, String>,
    emit: impl FnOnce(TorStatus),
) -> Result<T, String> {
    result.map_err(|error| {
        emit(TorStatus {
            state: "error",
            message: format!("TOR ERROR: {error}"),
        });
        error
    })
}

fn config_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("config.json"))
}

/// Scan `dir` for files whose extension is in `exts` (default: images).
fn scan_files(dir: &Path, exts: &[String]) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file()
            && p.extension()
                .and_then(|e| e.to_str())
                .map(|e| exts.iter().any(|x| x.eq_ignore_ascii_case(e)))
                .unwrap_or(false)
        {
            files.push(p);
        }
    }
    files.sort();
    Ok(files)
}

#[tauri::command]
fn list_engines() -> Vec<EngineInfo> {
    engines::registry()
        .into_iter()
        .map(|e| EngineInfo {
            id: e.id().to_string(),
            name: e.name().to_string(),
            options_schema: {
                // opsi mitigasi batch (jeda antar file + retry 403) dipasang
                // di sini sekali, berlaku untuk semua engine.
                let mut s = e.options_schema();
                s.extend(common_batch_options());
                s
            },
            input_exts: e.input_exts().iter().map(|s| s.to_string()).collect(),
        })
        .collect()
}

#[tauri::command]
fn scan_dir(path: String, exts: Option<Vec<String>>) -> Result<Vec<FileInfo>, String> {
    let exts = exts.unwrap_or_else(|| {
        ["jpg", "jpeg", "png", "webp"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    });
    Ok(scan_files(Path::new(&path), &exts)?
        .into_iter()
        .map(|p| {
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_string();
            let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            FileInfo {
                path: p.to_string_lossy().replace('\\', "/"),
                name: p
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("?")
                    .to_string(),
                size,
                ext,
            }
        })
        .collect())
}

fn needs_tor(options: &EngineOptions) -> bool {
    options.get("proxy_mode").and_then(|value| value.as_str()) == Some("tor")
}

fn inject_tor_addr(options: &mut EngineOptions, endpoint: &str) {
    options.insert("tor_addr".into(), serde_json::json!(endpoint));
}

fn needs_free(options: &EngineOptions) -> bool {
    options.get("proxy_mode").and_then(|value| value.as_str()) == Some("free")
}

/// Batas mode "free": engine hanya mengenal `direct|user|tor`. Kandidat proxy
/// gratis diambil di sini lalu opsi ditulis-ulang ke mode `user` beserta
/// `proxy_list`, sehingga engine mengonsumsinya lewat alur rotator yang sudah
/// ada. Daftar kosong atau gagal ambil menolak sebelum batch di-spawn.
async fn translate_free_mode<F, Fut>(options: &mut EngineOptions, fetcher: F) -> Result<(), String>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<Vec<Candidate>, String>>,
{
    let candidates = match fetcher().await {
        Ok(list) if !list.is_empty() => list,
        Ok(_) => return Err("tidak ada proxy gratis yang hidup".into()),
        Err(error) => return Err(format!("tidak ada proxy gratis yang hidup: {error}")),
    };
    let urls = candidates
        .into_iter()
        .map(|c| serde_json::Value::String(c.url))
        .collect();
    options.insert("proxy_list".into(), serde_json::Value::Array(urls));
    options.insert("proxy_mode".into(), serde_json::json!("user"));
    Ok(())
}

/// Fetcher produksi: client reqwest biasa + kunci HProxy dari konfigurasi.
async fn prepare_free_options(options: &mut EngineOptions, app: &AppHandle) -> Result<(), String> {
    let hproxy_api_key = config_path(app)
        .map(|path| config_load(&path).hproxy_api_key)
        .unwrap_or_default();
    let key = Some(hproxy_api_key.trim()).filter(|key| !key.is_empty());
    crate::net::http::ensure_crypto_provider();
    let http = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|error| error.to_string())?;
    translate_free_mode(options, || freeproxy::fetch_free_candidates(key, &http)).await
}

#[tauri::command]
async fn start_batch(
    app: AppHandle,
    files: Vec<String>,
    output: String,
    engine_id: String,
    mut options: EngineOptions,
    state: State<'_, BatchState>,
    licensing: State<'_, LicensingState>,
    tor: State<'_, TorManager>,
    resources: State<'_, TorResources>,
) -> Result<(), String> {
    if files.is_empty() {
        return Err("no files to process".into());
    }
    if output.trim().is_empty() {
        return Err("output folder is required".into());
    }
    if !engines::registry().iter().any(|e| e.id() == engine_id) {
        return Err(format!("engine not found: {engine_id}"));
    }
    let decision = licensing
        .manager
        // Trial admission is enforced per file inside the rolling worker pool;
        // a batch larger than the remaining quota may process its eligible
        // files, while the next file is never started after the last success.
        .preflight(&engine_id, 1)
        .await
        .map_err(|error| error.to_string())?;
    ensure_batch_allowed(&decision)?;
    let trial_gate = matches!(decision.state, LicenseState::Trial)
        .then(|| TrialStartGate::new(decision.remaining as usize));
    let batch_start = state.begin_start();
    if needs_tor(&options) {
        let _ = app.emit(
            "tor://status",
            TorStatus {
                state: "starting",
                message: "TOR STARTING".into(),
            },
        );
        let preparation = prepare_tor_inputs(
            resources.paths.clone(),
            app.path()
                .app_data_dir()
                .map_err(|error| format!("cannot resolve Tor data directory: {error}")),
        );
        let (paths, data_dir) = finish_tor_preparation(preparation, |status| {
            let _ = app.emit("tor://status", status);
        })?;
        let endpoint = finish_tor_preparation(tor.ensure_ready(paths, data_dir).await, |status| {
            let _ = app.emit("tor://status", status);
        })?;
        inject_tor_addr(&mut options, &endpoint);
        let _ = app.emit(
            "tor://status",
            TorStatus {
                state: "ready",
                message: "TOR READY".into(),
            },
        );
    }
    if needs_free(&options) {
        prepare_free_options(&mut options, &app).await?;
    }
    // Satu engine per worker dibangun dari registry yang sama; engine
    // stateless berbagi perilaku, state rotator tidak balapan antar worker.
    let engine_id_cl = engine_id.clone();
    let make_engines = move |worker: usize| {
        let _ = worker;
        engines::registry()
            .into_iter()
            .find(|e| e.id() == engine_id_cl)
            .expect("engine id tervalidasi di atas — unreachable backstop")
    };
    let files: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
    let out_dir = PathBuf::from(&output);
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;

    let (cancel, pause) = batch_start.into_controls(&state)?;
    let total = files.len();
    let emit_app = app.clone();
    let usage_manager = licensing.manager.clone();
    let usage_engine_id = engine_id.clone();
    tauri::async_runtime::spawn(async move {
        let (ok, fail) = run_batch_with_trial_gate(
            make_engines,
            files,
            &out_dir,
            &options,
            cancel,
            pause,
            trial_gate,
            move |ev| {
                if let batch::BatchEvent::FileDone { input, output, .. } = &ev {
                    if let Err(error) = usage_manager.record_success(
                        &usage_engine_id,
                        Path::new(input),
                        Path::new(output),
                    ) {
                        eprintln!("LICENSE USAGE ERROR: {error}");
                    }
                    let sync_manager = usage_manager.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = sync_manager.sync_pending_usage().await;
                    });
                }
                let _ = emit_app.emit("batch://event", ev);
            },
        )
        .await;
        let _ = app.emit(
            "batch://done",
            serde_json::json!({ "ok": ok, "fail": fail, "total": total }),
        );
    });
    Ok(())
}

fn ensure_batch_allowed(decision: &AccessDecision) -> Result<(), String> {
    if decision.allowed {
        Ok(())
    } else {
        Err(decision.message.clone())
    }
}

#[tauri::command]
async fn license_status(state: State<'_, LicensingState>) -> Result<LicenseStatus, String> {
    state
        .manager
        .status()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn license_purchase_url(state: State<'_, LicensingState>) -> Result<String, String> {
    state
        .manager
        .checkout_url()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn activate_license(
    state: State<'_, LicensingState>,
    license_key: String,
) -> Result<LicenseStatus, String> {
    state
        .manager
        .activate(license_key)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn refresh_license(state: State<'_, LicensingState>) -> Result<LicenseStatus, String> {
    state
        .manager
        .refresh()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn license_preflight(
    state: State<'_, LicensingState>,
    engine_id: String,
    requested_files: usize,
) -> Result<AccessDecision, String> {
    state
        .manager
        .preflight(&engine_id, requested_files)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn stat_files(files: Vec<String>) -> Vec<u64> {
    files
        .iter()
        .map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
        .collect()
}

#[tauri::command]
fn stop_batch(state: State<'_, BatchState>) {
    state.cancel_active();
}

#[tauri::command]
fn pause_batch(state: State<'_, BatchState>, paused: bool) {
    state.pause_active(paused);
}

#[tauri::command]
fn open_dir(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    std::process::Command::new("explorer")
        .arg(&path)
        .spawn()
        .map_err(|e| format!("cannot open folder: {e}"))?;
    Ok(())
}

#[tauri::command]
fn get_config(app: AppHandle) -> AppConfig {
    match config_path(&app) {
        Some(p) => config_load(&p),
        None => AppConfig::default(),
    }
}

#[tauri::command]
fn save_config(app: AppHandle, cfg: AppConfig) -> Result<(), String> {
    let p = config_path(&app).ok_or("cannot resolve app config dir")?;
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    config_save(&p, &cfg)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(TorManager::new())
        .setup(|app| {
            // Point the PngToSvg engine at the bundled portable Node
            // (resource dir in release; repo dir in dev when present).
            let node = [
                app.path()
                    .resolve("pngtosvg-runtime/node/node.exe", BaseDirectory::Resource)
                    .ok(),
                Some(PathBuf::from("pngtosvg-runtime/node/node.exe")),
                Some(PathBuf::from("src-tauri/pngtosvg-runtime/node/node.exe")),
            ]
            .into_iter()
            .flatten()
            .find(|p| p.exists());
            crate::engines::pngtosvg::set_bundled_node(node);
            let resource_root = app.path().resolve("", BaseDirectory::Resource).ok();
            let paths = std::env::current_dir()
                .map_err(|error| {
                    format!("cannot resolve working directory for bundled Tor: {error}")
                })
                .and_then(|cwd| resolve_runtime(resource_root.as_deref(), &cwd));
            app.manage(TorResources { paths });

            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| format!("cannot resolve licensing data directory: {error}"))?;
            app.manage(LicensingState::new(&app_data_dir).map_err(|error| error.to_string())?);

            Ok(())
        })
        .manage(BatchState::default())
        .invoke_handler(tauri::generate_handler![
            list_engines,
            scan_dir,
            stat_files,
            start_batch,
            stop_batch,
            pause_batch,
            open_dir,
            get_config,
            save_config,
            license_status,
            license_purchase_url,
            activate_license,
            refresh_license,
            license_preflight
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            if let Err(error) = app_handle.state::<TorManager>().shutdown() {
                eprintln!("TOR ERROR: shutdown failed: {error}");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::licensing::AccessDecision;

    #[test]
    fn tor_endpoint_injected_only_for_tor_mode() {
        let mut tor = EngineOptions::from([("proxy_mode".into(), serde_json::json!("tor"))]);
        assert!(needs_tor(&tor));
        inject_tor_addr(&mut tor, "socks5://127.0.0.1:19050");
        assert_eq!(tor["tor_addr"], "socks5://127.0.0.1:19050");

        let direct = EngineOptions::from([("proxy_mode".into(), serde_json::json!("direct"))]);
        assert!(!needs_tor(&direct));
    }

    #[test]
    fn denied_license_decision_stops_batch_before_side_effects() {
        let decision = AccessDecision::denied("pngtosvg", 0, "trial engine ini sudah habis");
        let error = ensure_batch_allowed(&decision).unwrap_err();
        assert!(error.contains("trial engine ini sudah habis"));
    }

    #[tokio::test]
    async fn free_mode_translates_to_user_with_fetched_list() {
        let mut options =
            EngineOptions::from([("proxy_mode".to_string(), serde_json::json!("free"))]);
        let fetcher = || {
            let cands = vec![
                freeproxy::Candidate {
                    url: "socks5://1.1.1.1:1080".into(),
                },
                freeproxy::Candidate {
                    url: "http://2.2.2.2:8080".into(),
                },
            ];
            Box::pin(std::future::ready(Ok(cands)))
                as std::pin::Pin<Box<dyn Future<Output = Result<Vec<_>, String>>>>
        };
        translate_free_mode(&mut options, fetcher).await.unwrap();
        assert_eq!(options["proxy_mode"], "user");
        let list = options["proxy_list"].as_array().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0], "socks5://1.1.1.1:1080");
    }

    #[tokio::test]
    async fn free_mode_rejects_when_no_healthy_candidate_message() {
        let mut options =
            EngineOptions::from([("proxy_mode".to_string(), serde_json::json!("free"))]);
        let fetcher = || {
            Box::pin(std::future::ready(Ok(Vec::new())))
                as std::pin::Pin<Box<dyn Future<Output = Result<Vec<_>, String>>>>
        };
        let err = translate_free_mode(&mut options, fetcher)
            .await
            .unwrap_err();
        assert!(err.contains("tidak ada proxy gratis yang hidup"), "{err}");
    }

    #[test]
    fn free_boundary_matches_only_free_mode() {
        let free = EngineOptions::from([("proxy_mode".into(), serde_json::json!("free"))]);
        assert!(needs_free(&free));

        let user = EngineOptions::from([("proxy_mode".into(), serde_json::json!("user"))]);
        let direct = EngineOptions::from([("proxy_mode".into(), serde_json::json!("direct"))]);
        let tor = EngineOptions::from([("proxy_mode".into(), serde_json::json!("tor"))]);
        assert!(!needs_free(&user));
        assert!(!needs_free(&direct));
        assert!(!needs_free(&tor));
    }

    #[test]
    fn tor_resource_preparation_error_emits_exact_error_status() {
        let root =
            std::env::temp_dir().join(format!("xix-lib-tor-preparation-{}", std::process::id()));
        let runtime = root.join("resources/tor-runtime");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(runtime.join("tor")).unwrap();
        std::fs::create_dir_all(runtime.join("data")).unwrap();
        std::fs::write(runtime.join("tor/tor.exe"), []).unwrap();
        std::fs::write(runtime.join("data/geoip"), []).unwrap();
        let result = prepare_tor_inputs(
            resolve_runtime(
                Some(&root.join("resources")),
                &root.join("missing-working-directory"),
            ),
            Ok(PathBuf::from("app-data")),
        );
        let mut statuses = Vec::new();

        let error = finish_tor_preparation(result, |status| statuses.push(status)).unwrap_err();

        assert!(error.contains("geoip6"), "{error}");
        assert_eq!(
            statuses,
            [TorStatus {
                state: "error",
                message: format!("TOR ERROR: {error}"),
            }]
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn tor_data_dir_preparation_error_emits_exact_error_status() {
        let result = prepare_tor_inputs(
            Ok(()),
            Err("cannot resolve Tor data directory: denied".into()),
        );
        let mut statuses = Vec::new();

        let error = finish_tor_preparation(result, |status| statuses.push(status)).unwrap_err();

        assert_eq!(error, "cannot resolve Tor data directory: denied");
        assert_eq!(
            statuses,
            [TorStatus {
                state: "error",
                message: "TOR ERROR: cannot resolve Tor data directory: denied".into(),
            }]
        );
    }

    #[test]
    fn stop_during_tor_bootstrap_cancels_the_same_batch_generation() {
        let state = BatchState::default();
        let start = state.begin_start();

        state.cancel_active();

        assert!(start.into_controls(&state).is_err());
    }
}
