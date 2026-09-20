# Standar UI Desktop XIXLabs

Dokumen ini adalah acuan untuk semua aplikasi desktop `XIX-*`. Implementasi
pertama ada di XIX-Vectorizer; aplikasi baru mengikuti aturan ini agar tetap
terlihat sebagai satu keluarga produk tanpa menghilangkan karakter masing-masing
aplikasi.

## 1. Identitas aplikasi

Setiap aplikasi desktop memiliki nama produk sendiri, tetapi memakai pola
identitas berikut:

| Area | Aturan | Contoh Vectorizer |
| --- | --- | --- |
| Nama paket | Nama aplikasi tanpa awalan `XIX-` | `Vectorizer` |
| Judul jendela | `<NamaAplikasi> by XIXLabs.net` | `Vectorizer by XIXLabs.net` |
| Title bar | Ikon XIXLabs, nama aplikasi, lalu byline | `VECTORIZER` · `by XIXLabs.net` |
| Ikon | Sumber yang sama untuk title bar dan paket installer | `XIX.svg` |
| Palet | Boleh berbeda per aplikasi; struktur dan bahasa visual tetap | Sunset glass |

Awalan `XIX-` tidak ditulis ulang pada nama aplikasi di title bar karena merek
XIXLabs sudah terlihat melalui ikon. Nama produk tetap boleh memakai `XIX-*` di
repository dan `product_id` server untuk menjaga identitas teknis.

## 2. Aset ikon

Sumber merek resmi saat ini:

`E:/Playground/XIXLabs/XIX-AnimotionV2/frontend/public/XIX.svg`

Setiap repository desktop harus menyimpan salinannya sendiri di
`src/assets/XIX.svg`. Build tidak boleh bergantung pada path repository lain.

Untuk Tauri, buat ikon platform dari aset lokal:

```powershell
npx tauri icon src/assets/XIX.svg --output src-tauri/icons
```

`src-tauri/tauri.conf.json` harus mengatur:

```json
{
  "productName": "NamaAplikasi",
  "app": {
    "windows": [
      { "title": "NamaAplikasi by XIXLabs.net" }
    ]
  },
  "bundle": {
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.ico",
      "icons/icon.icns"
    ]
  }
}
```

Title bar memakai aset yang sama melalui path frontend lokal:

```html
<img src="assets/XIX.svg" alt="XIXLabs" draggable="false">
<span>NamaAplikasi</span>
<span>by XIXLabs.net</span>
```

Area title bar tetap mempertahankan `data-tauri-drag-region`. Ikon paket,
ikon desktop shortcut, ikon installer, dan ikon title bar harus diperiksa pada
build bersih karena Windows dapat menyimpan cache ikon lama.

## 3. Modal lisensi

Lisensi tidak mengambil ruang dari area kerja utama. Toolbar menyediakan tombol
icon-only `KeyRound` di sebelah kanan Settings. Tombol wajib memiliki tooltip
dan `aria-label`, tetapi tidak menampilkan teks.

Urutan isi modal:

1. Badge dan status lisensi.
2. Label `Trial Limit:`.
3. Satu counter bersama, misalnya `TOTAL: 10/10`, yang berlaku untuk semua engine.
4. Kolom license key dan tombol `ACTIVATE`.
5. Bantuan singkat sesuai kondisi lisensi.
6. Tombol `Get License` di bagian bawah untuk membuka checkout publik aplikasi.

Dropdown engine diurutkan berdasarkan versi terendah ke tertinggi. Setiap label
menyertakan metode pemrosesan dalam superscript, misalnya `Vectorize V1
⁽ᴼⁿˡᶦⁿᵉ⁾`, `Vectorize V2 ⁽ᴼⁿˡᶦⁿᵉ⁾`, dan `Vectorize V3 ⁽ᴼᶠᶠˡᶦⁿᵉ⁾`.

Tombol `Get License` membuka browser default melalui Tauri opener. URL yang
ditanam di aplikasi harus berupa URL checkout publik yang sudah diverifikasi,
bukan halaman admin Mayar, URL webhook, API key, atau secret. Saat berpindah
dari sandbox ke production, hanya URL publik ini yang diganti bersama
konfigurasi produk yang sesuai.

Modal wajib mendukung:

- fokus awal ke kolom license key;
- tombol close, klik backdrop, dan tombol Escape;
- fokus kembali ke tombol `KeyRound` saat modal ditutup;
- navigasi Tab yang tetap berada di dalam modal;
- status loading, gagal membuka checkout, key kosong, key tidak valid, dan
  lisensi aktif/expired/revoked.

## 4. Kontrak lisensi desktop

Aturan ini berlaku untuk seluruh aplikasi desktop, termasuk aplikasi yang
memiliki beberapa engine:

- satu aplikasi memiliki satu `product_id` dan satu produk pembayaran;
- seluruh aplikasi memakai lisensi berlangganan bulanan;
- trial dihitung sepuluh file berhasil total lintas engine;
- file gagal, dibatalkan, atau retry idempoten tidak mengurangi kuota tambahan;
- satu lisensi hanya boleh aktif pada satu perangkat;
- perubahan perangkat dilakukan melalui reset HWID oleh admin, bukan otomatis
  dari aplikasi;
- lease online diperbarui melalui gateway dan boleh digunakan offline maksimal
  14 hari;
- setelah lease offline kedaluwarsa, pemrosesan dikunci sampai validasi online
  berhasil;
- penghapusan file lisensi tidak mengembalikan trial dan dapat meminta validasi
  ulang pada perangkat yang sama;
- aplikasi tidak menyimpan API key Mayar, client secret, private signing key,
  atau token admin.

Status penguncian harus tetap informatif: file dan pengaturan pengguna tetap
tersedia, tetapi pemrosesan menampilkan alasan dan langkah pemulihan.

## 5. Data yang perlu ditentukan per aplikasi

Sebelum implementasi aplikasi baru, lengkapi nilai berikut di dokumentasi
aplikasi dan gateway:

| Nilai | Contoh | Catatan |
| --- | --- | --- |
| `product_id` | `xix-vectorizer` | Stabil dan unik per aplikasi |
| Nama aplikasi | `Vectorizer` | Tanpa awalan pada judul jendela |
| Engine ID | `vectorize-v1` | Engine digunakan untuk memilih proses; seluruh engine berbagi counter trial |
| URL checkout sandbox | URL publik Mayar sandbox | Dipakai hanya untuk test |
| URL checkout production | URL publik production | Dipakai setelah cutover |
| Gateway URL | `https://payment.xixlabs.net` | Desktop tidak menghubungi Mayar langsung |
| Redirect checkout | URL HTTPS yang di-allowlist | Tidak boleh URL lokal pada production |

### Release, updater, dan installer publik

Setiap aplikasi desktop yang didistribusikan ke pengguna harus memiliki alur
rilis yang dapat diulang dan tidak bergantung pada komputer developer:

- Source boleh tetap berada di repository private, tetapi repository release
  harus public agar updater dan halaman aplikasi dapat mengambil asset tanpa
  login GitHub. Gunakan pola nama `<owner>/XIX-<App>-release`.
- GitHub Actions berjalan pada push tag versi `v*`, membangun installer Windows
  yang ditandatangani, lalu menerbitkan tiga hal: installer versioned,
  `.sig`, dan `latest.json`.
- Workflow juga mengunggah salinan installer dengan nama stabil
  `<App>-latest-x64-setup.exe`. Halaman aplikasi harus menunjuk ke:
  `https://github.com/<owner>/XIX-<App>-release/releases/latest/download/<App>-latest-x64-setup.exe`.
  Jangan menanam nomor versi di tombol download.
- Aplikasi Tauri menyimpan public signing key di konfigurasi, memakai endpoint
  `releases/latest/download/latest.json`, dan menyimpan private signing key
  hanya di GitHub Actions secret. Nama secret boleh terdokumentasi, nilainya
  tidak boleh.
- Saat startup, aplikasi memeriksa release terbaru. Jika ada, tampilkan
  pemberitahuan yang meminta persetujuan pengguna sebelum mengunduh dan
  memasang update. Jangan memasang update saat batch sedang berjalan; setelah
  pemasangan, aplikasi boleh restart otomatis.
- Versi pada manifest JavaScript, Cargo, konfigurasi Tauri, tag Git, dan nama
  release harus sama. Release pertama wajib diuji dari link direct installer,
  bukan hanya dari halaman release.

Untuk Vectorizer, repository release adalah
`mfahryf/XIX-Vectorizer-release`, metadata updater berada di
`https://github.com/mfahryf/XIX-Vectorizer-release/releases/latest/download/latest.json`,
dan link installer stabilnya adalah
`https://github.com/mfahryf/XIX-Vectorizer-release/releases/latest/download/Vectorizer-latest-x64-setup.exe`.

## 6. Checklist QA sebelum rilis

### Visual dan interaksi

- [ ] Title bar menampilkan ikon XIXLabs, nama aplikasi, dan `by XIXLabs.net`.
- [ ] Tidak ada `★ XIX-` atau awalan `XIX-` yang terduplikasi di title bar.
- [ ] Ikon shortcut, executable, installer, dan title bar berasal dari aset yang
      sama.
- [ ] Tombol `KeyRound` berada di kanan Settings dan tidak memiliki label visual.
- [ ] Modal lisensi menampilkan `Trial Limit:` sebelum counter total bersama.
- [ ] `Get License` berada di bagian paling bawah modal dan membuka browser.
- [ ] Modal tetap terbaca pada status trial, active, offline, expired, revoked,
      device conflict, dan error koneksi.
- [ ] Tab, Shift+Tab, Escape, close, backdrop, hover, focus, disabled, dan
      loading bekerja.

### Lisensi dan data

- [ ] Sepuluh file berhasil total lintas engine terhitung tepat sekali.
- [ ] Satu license key ditolak pada perangkat kedua sampai admin melakukan reset.
- [ ] Lease offline 14 hari dapat dipakai dan pemrosesan terkunci setelah habis.
- [ ] Online pada hari berikutnya memperbarui lease bila subscription masih aktif.
- [ ] Penghapusan file lisensi tidak mengembalikan trial.
- [ ] Reset HWID hanya dapat dilakukan melalui admin dan tercatat di audit log.
- [ ] Tidak ada secret Mayar atau private key di source, bundle, log, atau installer.

### Build

- [ ] `npm run test:ui` lulus.
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml` lulus.
- [ ] `git diff --check` bersih.
- [ ] `npm run build` menghasilkan executable dan installer target.
- [ ] Installer diuji pada build bersih atau mesin yang cache ikonnya sudah
      diperbarui.
- [ ] Repository release public, tag versi sudah dipush, dan GitHub Actions
      selesai tanpa error.
- [ ] `latest.json` dapat diakses tanpa login dan berisi URL installer serta
      signature yang tidak kosong.
- [ ] Link direct installer stabil mengembalikan status `200` dan mengunduh
      file Windows yang benar.
- [ ] Membuka aplikasi dengan release baru menampilkan pemberitahuan update;
      menolak pemberitahuan tidak merusak pemrosesan.

## 7. Checklist integrasi aplikasi baru

1. Salin `XIX.svg` ke `src/assets` repository aplikasi.
2. Terapkan title bar dan nama paket sesuai bagian Identitas aplikasi.
3. Generate ikon Tauri dan masukkan seluruh target ikon ke konfigurasi bundle.
4. Tambahkan tombol `KeyRound` dan modal lisensi mengikuti urutan standar.
5. Daftarkan engine ID untuk validasi proses dan `trial_quota: 10` sebagai
   satu kuota trial bersama di gateway.
6. Daftarkan satu produk Mayar untuk satu aplikasi.
7. Uji sandbox end-to-end: checkout, webhook, penerbitan key, aktivasi, renewal,
   offline, expiry, device conflict, dan reset HWID.
8. Jalankan checklist QA sebelum repository dan konfigurasi production dinyatakan
   siap.
9. Bila aplikasi ini akan punya halaman publik, sediakan pratinjau jendela pada
   bagian unduhan halaman tersebut. Pratinjau disalin dari aplikasi, bukan
   digambar ulang dengan perkiraan: ukuran jendela, warna, tinggi bilah, dan
   tulisan setiap sel diambil dari berkas aplikasi. Rinciannya di
   `XIX-Vectorizer-web/docs/DESKTOP-WEB-PAGE-STANDARD.md` bagian Pratinjau
   aplikasi desktop.
10. Buat workflow release dengan signing key di GitHub Actions, public
    repository release, `latest.json`, signature, dan stable installer alias.
11. Hubungkan tombol download halaman publik ke stable installer alias; simpan
    override staging hanya sebagai environment variable.
12. Uji update dari satu release ke release berikutnya, termasuk notifikasi,
    persetujuan pengguna, pembatalan, restart, dan kondisi batch sedang aktif.
13. Uji tutup-buka aplikasi dan mulai batch kedua. State lisensi, identitas
    perangkat, counter trial, dan lease yang masih berlaku harus tetap ada.

## 8. Kegagalan yang sudah pernah terjadi

Bagian ini mencatat kejadian nyata pada integrasi XIX-Vectorizer supaya tidak
terulang, dan supaya aplikasi desktop berikutnya mengenali gejalanya lebih
cepat. Padanan untuk aplikasi web ada di
`XIX-AnimotionV2/docs/WEB-APP-INTEGRATION-STANDARD.md`.

| Gejala di aplikasi | Penyebab | Penanganan |
| --- | --- | --- |
| `Mayar license code is not active` | Kode ada, tetapi statusnya di Mayar bukan aktif untuk produk yang dicek | Cocokkan UUID produk di katalog dengan produk tempat kode itu diterbitkan |
| `Mayar software license product ID mismatch` | Katalog aplikasi menunjuk produk Mayar yang berbeda dari produk penerbit kode | Samakan `mayar_product_id` di katalog dengan produk yang benar-benar dipakai checkout |
| `There's no license with code ... and product id ... registered in user ...` | Kode diterbitkan akun atau produk lain daripada yang dicek gateway | Gunakan pasangan akun, kunci, dan UUID produk yang berasal dari satu lingkungan yang sama |
| `invalid_license` | Kode tidak dikenal untuk produk tersebut, atau sudah tidak berlaku menurut provider | Periksa status kode di dasbor Mayar, lalu katalog gateway |
| `device_conflict` | Kode sudah terikat ke perangkat lain | Reset perangkat lewat admin; aplikasi tidak boleh membuat binding kedua sendiri |
| `subscription_expired` padahal status provider masih aktif | Status provider dan langganan bulanan XIXLabs adalah dua hal berbeda | Perpanjang langganan lewat pembayaran; status provider tidak memperpanjang hak bulanan |
| `license_revoked` | Lisensi dicabut oleh admin | Hubungi admin; aplikasi menampilkan alasan dan langkah pemulihan |
| Lisensi hilang setelah aplikasi dibuka kembali | State lokal hanya ditulis langsung atau cache rusak tanpa backup | Gunakan penyimpanan terlindungi dengan tulis atomik dan backup; uji tutup-buka pada perangkat yang sama |
| Error jam muncul saat batch kedua setelah batch pertama selesai | Timestamp lease historis dipakai sebagai waktu server terkini | Hanya waktu server top-level dari respons status yang boleh memperbarui watermark waktu; timestamp lease hanya untuk validasi lease |
| Tombol download berhenti di halaman release | Link memakai halaman release atau nama installer versioned | Publikasikan stable installer alias pada setiap release dan gunakan `releases/latest/download/<stable-name>` |
| Update tersedia tetapi tidak terpasang | Public key, endpoint, asset, atau signature tidak cocok | Cocokkan public key dengan private signing secret, cek `latest.json`, dan uji installer direct sebelum publish |
| `jam perangkat mundur dari waktu server tepercaya` | Jam perangkat berada di belakang waktu server terakhir yang pernah dicatat aplikasi | Sinkronkan jam Windows, lalu jalankan validasi online |
| `layanan lisensi tidak tersedia: server error` | Gateway atau provider sedang gagal sementara | Bukan masalah lisensi pengguna; coba lagi, dan lease yang masih berlaku tetap dapat dipakai offline |
| Kode lisensi selamanya berstatus sedang diproses di halaman Mayar | Mayar belum menerbitkan kode untuk transaksi itu | Periksa transaksi di dasbor Mayar; ini bukan kegagalan aplikasi |

### Catatan waktu perangkat

Aplikasi menyimpan waktu server terakhir yang dipercaya, dan menolak jam yang
berada di belakang nilai itu. Pemulihan online hanya menerima selisih paling
besar lima menit antara jam perangkat dan jam server.

Respons status dapat membawa lease yang diterbitkan pada waktu sebelumnya.
Timestamp lease tersebut bukan waktu server saat ini dan tidak boleh menggantikan
watermark `last_server_time`. Kesalahan ini dapat membuat batch pertama berhasil,
tetapi batch berikutnya salah dianggap mengalami clock rollback.

Konsekuensinya, jam yang tampak benar belum tentu cukup: bila jam perangkat
berada lebih dari lima menit di belakang jam server, menyambung ke internet
sendiri tidak memperbaiki keadaan. Sinkronkan jam perangkat lebih dulu, baru
jalankan validasi ulang.
