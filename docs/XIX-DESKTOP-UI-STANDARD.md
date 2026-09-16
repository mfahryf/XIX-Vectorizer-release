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
3. Counter setiap engine, misalnya `V1: 5/5`, `V2: 5/5`, dan `V3: 5/5`.
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
- trial dihitung lima file berhasil per engine;
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
| Engine ID | `vectorize-v1` | Setiap engine memiliki counter trial sendiri |
| URL checkout sandbox | URL publik Mayar sandbox | Dipakai hanya untuk test |
| URL checkout production | URL publik production | Dipakai setelah cutover |
| Gateway URL | `https://payment.xixlabs.net` | Desktop tidak menghubungi Mayar langsung |
| Redirect checkout | URL HTTPS yang di-allowlist | Tidak boleh URL lokal pada production |

## 6. Checklist QA sebelum rilis

### Visual dan interaksi

- [ ] Title bar menampilkan ikon XIXLabs, nama aplikasi, dan `by XIXLabs.net`.
- [ ] Tidak ada `★ XIX-` atau awalan `XIX-` yang terduplikasi di title bar.
- [ ] Ikon shortcut, executable, installer, dan title bar berasal dari aset yang
      sama.
- [ ] Tombol `KeyRound` berada di kanan Settings dan tidak memiliki label visual.
- [ ] Modal lisensi menampilkan `Trial Limit:` sebelum counter engine.
- [ ] `Get License` berada di bagian paling bawah modal dan membuka browser.
- [ ] Modal tetap terbaca pada status trial, active, offline, expired, revoked,
      device conflict, dan error koneksi.
- [ ] Tab, Shift+Tab, Escape, close, backdrop, hover, focus, disabled, dan
      loading bekerja.

### Lisensi dan data

- [ ] Lima file berhasil per engine terhitung tepat sekali.
- [ ] Satu license key ditolak pada perangkat kedua sampai admin melakukan reset.
- [ ] Lease offline 14 hari dapat dipakai dan pemrosesan terkunci setelah habis.
- [ ] Online pada hari berikutnya memperbarui lease bila subscription masih aktif.
- [ ] Penghapusan file lisensi tidak mengembalikan trial.
- [ ] Reset HWID hanya dapat dilakukan melalui admin dan tercatat di audit log.
- [ ] Tidak ada secret Mayar atau private key di source, bundle, log, atau installer.

### Build

- [ ] `npm run test:ui` lulus.
- [ ] `git diff --check` bersih.
- [ ] `npm run build` menghasilkan executable dan installer target.
- [ ] Installer diuji pada build bersih atau mesin yang cache ikonnya sudah
      diperbarui.

## 7. Checklist integrasi aplikasi baru

1. Salin `XIX.svg` ke `src/assets` repository aplikasi.
2. Terapkan title bar dan nama paket sesuai bagian Identitas aplikasi.
3. Generate ikon Tauri dan masukkan seluruh target ikon ke konfigurasi bundle.
4. Tambahkan tombol `KeyRound` dan modal lisensi mengikuti urutan standar.
5. Daftarkan engine ID dan kuota lima file per engine di gateway.
6. Daftarkan satu produk Mayar untuk satu aplikasi.
7. Uji sandbox end-to-end: checkout, webhook, penerbitan key, aktivasi, renewal,
   offline, expiry, device conflict, dan reset HWID.
8. Jalankan checklist QA sebelum repository dan konfigurasi production dinyatakan
   siap.

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
| `jam perangkat mundur dari waktu server tepercaya` | Jam perangkat berada di belakang waktu server terakhir yang pernah dicatat aplikasi | Sinkronkan jam Windows, lalu jalankan validasi online |
| `layanan lisensi tidak tersedia: server error` | Gateway atau provider sedang gagal sementara | Bukan masalah lisensi pengguna; coba lagi, dan lease yang masih berlaku tetap dapat dipakai offline |
| Kode lisensi selamanya berstatus sedang diproses di halaman Mayar | Mayar belum menerbitkan kode untuk transaksi itu | Periksa transaksi di dasbor Mayar; ini bukan kegagalan aplikasi |

### Catatan waktu perangkat

Aplikasi menyimpan waktu server terakhir yang dipercaya, dan menolak jam yang
berada di belakang nilai itu. Pemulihan online hanya menerima selisih paling
besar lima menit antara jam perangkat dan jam server.

Konsekuensinya, jam yang tampak benar belum tentu cukup: bila jam perangkat
berada lebih dari lima menit di belakang jam server, menyambung ke internet
sendiri tidak memperbaiki keadaan. Sinkronkan jam perangkat lebih dulu, baru
jalankan validasi ulang.
