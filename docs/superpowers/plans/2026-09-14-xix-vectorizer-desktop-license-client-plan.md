# XIX-Vectorizer Desktop License Client Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mengintegrasikan XIX-Vectorizer dengan layanan lisensi pusat XIXLabs sehingga trial, license key bulanan, satu device binding, lease offline 14 hari, dan state penguncian berjalan konsisten di aplikasi desktop.

**Architecture:** Aplikasi Tauri membuat identitas perangkat Ed25519 dan menyimpan private key serta lease melalui DPAPI Windows. Modul licensing terpisah melakukan preflight sebelum batch, mencatat file trial yang berhasil, dan menyinkronkan usage ke `XIX-Payment-Gateway`. UI hanya menampilkan status dari modul Rust; engine vectorization tidak menyimpan aturan lisensi dan tidak menghubungi Mayar.

**Tech Stack:** Tauri 2, Rust 2021, reqwest/rustls, serde/serde_json, Ed25519, Windows DPAPI, tokio, vanilla JavaScript, Node test runner, Cargo tests.

> **Catatan implementasi terbaru (2026-09-20):** Trial client telah disederhanakan
> menjadi satu penghitung total 10 file berhasil lintas engine. Rujukan untuk
> perilaku yang berlaku adalah `docs/DESKTOP-LICENSING-CONTRACT.md`; contoh
> lama yang memisahkan counter per engine di bawah ini bersifat historis.

## Global Constraints

- `product_id` desktop adalah `xix-vectorizer`.
- ID engine yang dipakai oleh kontrak adalah `vectorize-v1`, `vectorize-v2`, dan `pngtosvg`; `pngtosvg` ditampilkan sebagai Vectorize V3.
- Trial otomatis dimulai ketika file pertama akan diproses, bukan ketika aplikasi sekadar dibuka.
- Trial memiliki 10 file berhasil total dan dipakai bersama oleh semua engine.
- Engine tetap dipilih untuk proses, tetapi tidak memiliki kuota trial terpisah.
- License key bulanan membuka seluruh engine.
- Satu license key hanya boleh memiliki satu device binding aktif.
- Lease lokal berlaku paling lama 14 hari dan tidak melewati masa langganan.
- Trial pertama memerlukan koneksi online; kuota yang sudah diterbitkan dapat dipakai offline dengan counter aman.
- File gagal, dibatalkan, atau retry yang sama tidak boleh mengurangi kuota dua kali.
- Penghapusan cache lease tidak mengulang trial; penghapusan identitas perangkat memerlukan pemulihan admin.
- Data hasil, input, playlist, dan pengaturan kerja tidak boleh dihapus atau disimpan di database lisensi.
- Tidak ada Google Auth, API key Mayar, client secret, atau secret signing private dalam binary desktop.
- Jika status tidak dapat dibuktikan, hanya pemrosesan yang dikunci; halaman aktivasi, bantuan, pengaturan, dan file lama tetap dapat dibuka.

## Prasyarat layanan pusat

Implementasi desktop bergantung pada kontrak di:

`E:/Playground/XIXLabs/XIX-Payment-Gateway/docs/superpowers/plans/2026-09-14-central-desktop-licensing-service-plan.md`

Sebelum integrasi production, endpoint berikut harus tersedia pada base URL
`https://payment.xixlabs.net` atau URL development yang dipakai test:

- `POST /v1/desktop/trial/claim`
- `POST /v1/desktop/license/activate`
- `GET /v1/desktop/license/status`
- `POST /v1/desktop/license/renew`
- `POST /v1/desktop/usage/record`

Response harus menyediakan `license_state`, `subscription_expires_at`,
`lease_expires_at`, `device_state`, `trial_remaining`, `reason`,
`server_time`, `key_id`, dan signature lease.

## File map

- Create: `src-tauri/src/licensing/mod.rs` untuk facade dan state machine.
- Create: `src-tauri/src/licensing/models.rs` untuk state, request, response, dan lease.
- Create: `src-tauri/src/licensing/device.rs` untuk pembuatan dan pemuatan identitas perangkat.
- Create: `src-tauri/src/licensing/storage.rs` untuk file terenkripsi DPAPI dan local usage ledger.
- Create: `src-tauri/src/licensing/client.rs` untuk transport HTTPS dan endpoint pusat.
- Create: `src-tauri/src/licensing/usage.rs` untuk counter trial, deduplication, dan queue sinkronisasi.
- Create: `src-tauri/src/licensing/error.rs` untuk error yang aman ditampilkan UI.
- Create: `src-tauri/src/licensing/tests.rs` untuk unit test modul lisensi.
- Create: `licensing-ui.test.js` untuk test state UI tanpa menjalankan Tauri.
- Create: `src/licensing-ui.js` untuk fungsi presentasi state dan counter trial yang murni.
- Modify: `src-tauri/Cargo.toml` untuk dependency Ed25519 dan random source.
- Modify: `src-tauri/src/lib.rs` untuk register state, commands, preflight, dan pencatatan `FileDone`.
- Modify: `src-tauri/src/batch.rs` untuk menyertakan input path pada event `FileDone`.
- Modify: `src/index.html` untuk panel aktivasi, status lisensi, counter engine, dan bantuan.
- Modify: `src/main.js` untuk startup status, gate Start, activation actions, dan event refresh.
- Modify: `src/style.css` untuk style panel lisensi yang konsisten dengan UI glass/Winamp.
- Modify: `package.json` untuk script test UI.
- Modify: `README.md` untuk aktivasi, trial, offline, device recovery, dan troubleshooting.
- Modify: `.gitignore` jika diperlukan agar local license cache dan device identity tidak masuk Git.

## Interfaces antar-task

Modul Rust harus mengekspos API internal berikut:

```rust
pub const PRODUCT_ID: &str = "xix-vectorizer";

pub async fn status(&self, app: &AppHandle) -> Result<LicenseStatus, LicenseError>;
pub async fn activate(&self, app: &AppHandle, license_key: String) -> Result<LicenseStatus, LicenseError>;
pub async fn refresh(&self, app: &AppHandle) -> Result<LicenseStatus, LicenseError>;
pub async fn preflight(&self, app: &AppHandle, engine_id: &str, requested_files: usize) -> Result<AccessDecision, LicenseError>;
pub fn record_success(&self, engine_id: &str, input: &Path, output: &Path) -> Result<(), LicenseError>;
pub async fn sync_pending_usage(&self, app: &AppHandle) -> Result<(), LicenseError>;
```

`AccessDecision` memuat `allowed`, `state`, `engine_id`, `remaining`, dan
`message`. `LicenseStatus` harus serializable untuk command Tauri dan tidak
memuat private key atau raw license key.

### Task 1: Model state, engine mapping, and local storage

**Files:**
- Create: `src-tauri/src/licensing/mod.rs`
- Create: `src-tauri/src/licensing/models.rs`
- Create: `src-tauri/src/licensing/storage.rs`
- Create: `src-tauri/src/licensing/usage.rs`
- Create: `src-tauri/src/licensing/error.rs`
- Create: `src-tauri/src/licensing/tests.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `.gitignore`

**Interfaces:**
- Consumes: `secure::dpapi::{protect, unprotect}` dan engine registry dari repository saat ini.
- Produces: `LicenseState`, `EngineTrial`, `LocalLease`, `DeviceIdentity`, `AccessDecision`, dan storage methods untuk Task 2–5.

- [ ] **Step 1: Tulis unit test gagal untuk state dan mapping.**

Uji mapping tiga engine aktual, counter total 10 file, lock setelah total habis, dan status
aplikasi ketika semua counter habis.

```rust
#[test]
fn trial_uses_real_engine_ids_and_locks_only_exhausted_engine() {
    let mut trial = TrialState::new(["vectorize-v1", "vectorize-v2", "pngtosvg"]);
    assert_eq!(trial.remaining("pngtosvg"), 5);
    for index in 0..5 {
        assert!(trial.record_success("pngtosvg", &format!("usage-{index}")));
    }
    assert_eq!(trial.remaining("pngtosvg"), 0);
    assert!(trial.is_locked("pngtosvg"));
    assert_eq!(trial.remaining("vectorize-v1"), 5);
}
```

- [ ] **Step 2: Jalankan test RED.**

Run: `cargo test --manifest-path src-tauri/Cargo.toml licensing::tests`

Expected: gagal karena modul licensing belum ada.

- [ ] **Step 3: Implementasikan model dan counter idempoten.**

Gunakan `vectorize-v1`, `vectorize-v2`, dan `pngtosvg` sebagai nilai kontrak.
`TrialState::record_success` menerima `usage_event_id` dan mengabaikan event
yang sama dua kali. Counter tidak boleh turun di bawah nol. `preflight` menolak
batch jika `requested_files > remaining` untuk engine trial dan mengembalikan
pesan jumlah file yang masih tersedia; lisensi aktif tidak memiliki batas ini.

- [ ] **Step 4: Implementasikan local lease and usage ledger.**

Simpan dua blob terpisah di app data directory: identitas perangkat dan lease
licensing. Lindungi keduanya dengan DPAPI memakai file sementara yang ditulis
atomically. Local usage ledger menyimpan event ID, engine ID, input fingerprint,
output fingerprint, waktu lokal, dan status sinkronisasi. Jangan simpan raw key
atau private key dalam `config.json` yang saat ini menyimpan pengaturan UI.

- [ ] **Step 5: Tambahkan dependency dan aturan Git ignore.**

Tambahkan `ed25519-dalek` dengan fitur signing yang diperlukan dan random source
yang didukung Windows. Pastikan pola berikut diabaikan:

```gitignore
src-tauri/target/
*.license
*.lease
device-identity*
license-cache*
usage-ledger*
```

- [ ] **Step 6: Jalankan test GREEN dan commit.**

Run: `cargo test --manifest-path src-tauri/Cargo.toml licensing::tests`

Expected: mapping engine, counter, deduplication, expiry, dan atomic local
storage lulus.

```text
git add src-tauri/Cargo.toml src-tauri/src/licensing .gitignore
git commit -m "feat: add vectorizer licensing state and local ledger"
```

### Task 2: Device identity, signed requests, and central client

**Files:**
- Modify: `src-tauri/src/licensing/device.rs`
- Modify: `src-tauri/src/licensing/storage.rs`
- Modify: `src-tauri/src/licensing/client.rs`
- Modify: `src-tauri/src/licensing/models.rs`
- Modify: `src-tauri/src/licensing/tests.rs`
- Modify: `src-tauri/src/secure/dpapi.rs` only if the current wrapper needs a typed error/helper

**Interfaces:**
- Consumes: model and storage dari Task 1.
- Produces: `DeviceIdentityStore::load_or_create`, `LicenseClient`, canonical request signing, lease validation, dan error mapping untuk Task 3–5.

- [ ] **Step 1: Tulis test gagal untuk device identity and lease validation.**

Uji bahwa dua kali load menghasilkan public key dan fingerprint yang sama,
private key tidak sama dengan blob terenkripsi, signature request berubah jika
challenge atau payload berubah, dan lease yang sudah lewat ditolak.

- [ ] **Step 2: Jalankan test RED.**

Run: `cargo test --manifest-path src-tauri/Cargo.toml licensing::tests::device`

Expected: gagal karena identity store dan client belum ada.

- [ ] **Step 3: Implementasikan identitas perangkat.**

Saat pertama kali dibutuhkan, buat Ed25519 key pair. Simpan private key melalui
DPAPI pada file lokal yang tidak sama dengan `config.json`; simpan public key,
fingerprint, dan registration ID dalam metadata yang juga dilindungi. Jangan
mengirim private key ke server. Jika blob tidak dapat dibuka, kembalikan error
`device_identity_lost` tanpa membuat binding baru secara otomatis.

- [ ] **Step 4: Implementasikan client HTTPS.**

Gunakan base URL production `https://payment.xixlabs.net`; sediakan override
development hanya melalui build/test configuration. Client harus:

1. Meminta challenge atau claim trial sesuai kontrak gateway.
2. Menandatangani canonical JSON dengan private key perangkat.
3. Mengirim `product_id`, versi aplikasi dari Tauri config, public key,
   registration ID, challenge, dan signature.
4. Memverifikasi signature lease dengan public key gateway yang ditanam sebagai
   verification key, bukan private key.
5. Memetakan HTTP 401/403/409/410/429/5xx ke error aplikasi yang aman.

- [ ] **Step 5: Implementasikan lease validation.**

Gunakan waktu server pada lease. Lease valid jika signature, product ID,
device fingerprint, subscription expiry, dan lease expiry semuanya sesuai.
`lease_expiry` tidak boleh melebihi `subscription_expiry`. Perubahan jam lokal
tidak boleh memperpanjang lease.

- [ ] **Step 6: Jalankan test GREEN dan commit.**

Run: `cargo test --manifest-path src-tauri/Cargo.toml licensing::tests`

Expected: identity persistence, signature, tamper rejection, expiry, dan
error mapping lulus tanpa menampilkan key material ke output.

```text
git add src-tauri/src/licensing src-tauri/src/secure/dpapi.rs
git commit -m "feat: add signed desktop license client"
```

### Task 3: Tauri commands and startup state

**Files:**
- Modify: `src-tauri/src/licensing/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/licensing/tests.rs`

**Interfaces:**
- Consumes: `LicenseClient`, storage, and state model dari Task 1–2.
- Produces: Tauri commands `license_status`, `activate_license`, `refresh_license`, dan `license_preflight` untuk frontend dan batch gate.

- [ ] **Step 1: Tulis test command/state yang gagal.**

Uji bahwa status awal tidak memuat secret, activation success membuka semua
engine, device conflict menghasilkan state khusus, dan lease expired tidak
dianggap aktif.

- [ ] **Step 2: Jalankan test RED.**

Run: `cargo test --manifest-path src-tauri/Cargo.toml licensing::tests::commands`

Expected: gagal karena command dan manager belum terdaftar.

- [ ] **Step 3: Tambahkan `LicensingState` ke Tauri builder.**

Buat satu manager per proses aplikasi, lalu daftarkan melalui `.manage(...)`
di `run()`. Inisialisasi tidak boleh membuat jaringan blocking sebelum window
siap. Startup hanya membaca cache; refresh dilakukan async setelah UI mulai.

Tambahkan command berikut ke `tauri::generate_handler!`:

```rust
license_status,
activate_license,
refresh_license,
license_preflight,
```

`activate_license` menerima raw key hanya di memory request dan mengembalikan
status tanpa mengembalikan kembali raw key. `license_preflight` tidak boleh
menjalankan engine atau membuat file output.

- [ ] **Step 4: Implementasikan startup refresh yang aman.**

Jika cache lease valid, kembalikan `licensed-offline` segera. Jika cache hampir
habis atau tidak ada, coba refresh async. Error jaringan tidak menggantikan
status cache valid. Cache hilang pada perangkat yang sama memicu validasi online;
identitas perangkat hilang memicu `device_identity_lost`.

- [ ] **Step 5: Jalankan test GREEN dan commit.**

Run: `cargo test --manifest-path src-tauri/Cargo.toml licensing::tests`

Expected: semua state dan command lulus.

```text
git add src-tauri/src/lib.rs src-tauri/src/licensing
git commit -m "feat: expose vectorizer license commands"
```

### Task 4: Batch preflight and successful-file accounting

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/batch.rs`
- Modify: `src-tauri/src/licensing/usage.rs`
- Modify: `src-tauri/src/licensing/tests.rs`
- Modify: `src/main.js` only to consume any changed event field without breaking existing handlers

**Interfaces:**
- Consumes: `license_preflight`, local ledger, dan `BatchEvent` dari Task 1–3.
- Produces: batch yang tidak dimulai ketika locked, trial usage yang dihitung tepat satu kali, dan queued sync setelah output berhasil.

- [ ] **Step 1: Tulis regression test gagal untuk batch gate.**

Tambahkan test bahwa batch trial dengan sisa dua file menolak permintaan lima
file tanpa membuat output, batch berlisensi menerima jumlah berapa pun, dan
`FileDone` membawa input serta output yang dibutuhkan untuk usage fingerprint.

```rust
#[test]
fn file_done_contains_input_for_usage_deduplication() {
    let event = BatchEvent::FileDone {
        input: "C:/in/a.png".into(),
        name: "a.png".into(),
        output: "C:/out/a.svg".into(),
    };
    match event {
        BatchEvent::FileDone { input, output, .. } => {
            assert_eq!(input, "C:/in/a.png");
            assert!(output.ends_with("a.svg"));
        }
        _ => unreachable!(),
    }
}
```

- [ ] **Step 2: Jalankan test RED.**

Run: `cargo test --manifest-path src-tauri/Cargo.toml batch licensing::tests`

Expected: gagal karena event belum memiliki input dan `start_batch` belum
memanggil license preflight.

- [ ] **Step 3: Tambahkan preflight sebelum side effect batch.**

Di `start_batch`, validasi daftar file, engine ID, dan lisensi sebelum
menyalakan Tor, mengambil proxy, membuat output directory, atau memulai worker.
Untuk trial, `requested_files` harus tidak melebihi sisa engine. Untuk lease
expired, kembalikan error yang dapat dipetakan UI tanpa membuat batch state
aktif.

- [ ] **Step 4: Tambahkan input ke `BatchEvent::FileDone`.**

Pertahankan field `name` dan `output` yang dipakai frontend, lalu tambahkan
`input: String` dari path sumber. Karena event dikirim ke JavaScript, handler
lama yang hanya membaca `name` dan `output` tetap kompatibel.

- [ ] **Step 5: Catat keberhasilan dan sinkronisasi.**

Pada callback `FileDone`, hitung fingerprint input dan output, buat event ID
deterministik untuk engine + input fingerprint + batch generation, lalu tulis
ke local ledger sebelum melakukan sync. Jalankan sync async setelah event
diterima; jika offline, event tetap queued. Usage server yang sama tidak boleh
mengurangi trial dua kali.

Jika output gagal diverifikasi atau file tidak ada, jangan mencatat `FileDone`.
Jangan mengurangi trial untuk `FileFail` atau cancel.

- [ ] **Step 6: Jalankan test GREEN dan commit.**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Expected: seluruh batch test lama tetap lulus, ditambah gate, event input,
counter sukses, duplicate usage, dan offline queue.

```text
git add src-tauri/src/lib.rs src-tauri/src/batch.rs src-tauri/src/licensing src/main.js
git commit -m "feat: enforce license gate in vectorizer batches"
```

### Task 5: Activation UI and state presentation

**Files:**
- Create: `src/licensing-ui.js`
- Create: `licensing-ui.test.js`
- Modify: `src/index.html`
- Modify: `src/main.js`
- Modify: `src/style.css`
- Modify: `package.json`

**Interfaces:**
- Consumes: Tauri commands and `LicenseStatus` dari Task 3–4.
- Produces: UI aktivasi, satu trial counter total, warning lease, dan locked state yang tidak mengganggu file lama.

- [ ] **Step 1: Tulis test UI gagal.**

Buat fungsi murni `deriveLicenseView(status)` dan uji label berikut:

```javascript
test("expired lease locks processing but leaves recovery actions", () => {
  const view = deriveLicenseView({
    license_state: "expired-offline",
    trial_remaining_by_engine: { "vectorize-v1": 0, "vectorize-v2": 2, pngtosvg: 5 },
  });
  assert.equal(view.canProcess, false);
  assert.equal(view.showActivation, true);
  assert.equal(view.message, "Hubungkan internet untuk memvalidasi lisensi.");
});
```

- [ ] **Step 2: Jalankan test RED.**

Run: `npm run test:ui`

Expected: gagal karena helper dan script belum ada.

- [ ] **Step 3: Implementasikan view model murni.**

`deriveLicenseView` mengembalikan `canProcess`, `showActivation`, `badge`,
`message`, `engineCounters`, dan `offlineDaysRemaining`. Gunakan pesan berbeda
untuk trial aktif, engine trial habis, lisensi aktif, offline valid, lease
expired, subscription expired, revoke, dan device conflict.

- [ ] **Step 4: Tambahkan panel lisensi ke markup.**

Tambahkan elemen dengan ID stabil berikut ke `src/index.html`:

```text
license-badge
license-panel
license-key
license-status
license-activate
license-refresh
license-trial-v1
license-trial-v2
license-trial-v3
license-help
```

`license-trial-total` menampilkan counter total untuk seluruh engine. Jangan
menampilkan HWID mentah atau private key.

- [ ] **Step 5: Hubungkan startup dan tombol Start.**

Pada startup, panggil `license_status`, render view, lalu panggil refresh bila
dibutuhkan. Tombol Start menjalankan `license_preflight` sebelum
`start_batch`. Tombol aktivasi memanggil `activate_license`; tombol refresh
memanggil `refresh_license`. Saat status berubah, render ulang badge dan counter.

Jangan menutup panel settings atau menghapus playlist ketika lisensi terkunci.
Pesan lock harus menjelaskan tindakan berikutnya tanpa memaparkan error teknis.

- [ ] **Step 6: Tambahkan style dan test GREEN.**

Gunakan komponen glass/Winamp yang sudah ada. State terkunci harus terlihat
jelas tetapi tidak menyembunyikan hasil lama. Tambahkan script berikut:

```json
"test:ui": "node --test licensing-ui.test.js"
```

Run: `npm run test:ui`

Expected: seluruh test view trial, active, offline, expired, revoked, dan
device conflict lulus.

```text
git add src/licensing-ui.js licensing-ui.test.js src/index.html src/main.js src/style.css package.json
git commit -m "feat: add vectorizer license activation UI"
```

### Task 6: End-to-end verification and release handoff

**Files:**
- Modify: `README.md`
- Modify: `src-tauri/src/licensing/tests.rs` if regression coverage needs a missing case
- Inspect: `src-tauri/tauri.conf.json`, `.gitignore`, dan build resource list
- Test: seluruh test Rust dan UI

**Interfaces:**
- Consumes: client, command, batch gate, usage ledger, dan UI dari Task 1–5.
- Produces: bukti bahwa Vectorizer dapat trial, aktivasi, offline, recover, dan lock sesuai desain.

- [ ] **Step 1: Tambahkan checklist manual yang dapat diverifikasi.**

README harus memuat langkah trial, aktivasi key dari email Mayar, satu-device
binding, penggantian perangkat melalui admin, penghapusan cache, masa offline
14 hari, dan arti setiap state. Jangan menaruh license key atau endpoint secret
asli di dokumentasi.

- [ ] **Step 2: Jalankan test otomatis.**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Expected: seluruh unit dan regression test Rust lulus.

Run: `npm run test:ui`

Expected: seluruh test UI lisensi lulus.

Run: `npm run build`

Expected: Tauri build selesai tanpa error dan resource `pngtosvg-runtime` serta
`tor-runtime` tetap terbundel.

- [ ] **Step 3: Jalankan smoke test dengan gateway development.**

Gunakan endpoint test dan key signing test, bukan production secret. Verifikasi
urutan: fresh install → first file claim → sepuluh file lintas engine → trial lock →
license activation → second device conflict → simulated offline lease → lease
expiry → online refresh → cache deletion recovery → device identity deletion
recovery.

- [ ] **Step 4: Jalankan pemeriksaan keamanan dan Git.**

Run: `git diff --check`

Run: `rg -n -i "MAYAR_API_KEY|CLIENT_SECRET|PRIVATE_KEY|license key|XIX_LICENSE" src src-tauri README.md .gitignore`

Expected: tidak ada secret produksi atau raw license key; nama konfigurasi
yang muncul hanya berupa dokumentasi dummy dan variable yang memang dibaca
oleh client.

Run: `git status --short --branch`

Expected: hanya perubahan yang direncanakan dan tidak ada local lease, device
identity, usage ledger, build output, atau database dalam Git.

- [ ] **Step 5: Commit release handoff.**

```text
git add README.md src-tauri/src/licensing src-tauri/src/lib.rs src-tauri/src/batch.rs src/index.html src/style.css package.json .gitignore
git commit -m "test: verify vectorizer desktop licensing flow"
```

- [ ] **Step 6: Handoff deployment dan pilot.**

Setelah gateway siap di Coolify, buat satu produk Mayar untuk
`xix-vectorizer-monthly`, uji satu transaksi sandbox, kirim key ke email test,
aktivasi pada satu komputer, dan catat hasilnya. Jangan mengubah engine online
atau menghapus data pengguna sebagai bagian dari aktivasi lisensi.
