# Kontrak Lisensi Desktop XIX-Vectorizer

Dokumen ini adalah panduan implementasi client desktop. Sumber kebenaran
server berada di `XIX-Payment-Gateway/docs/API.md`. Aplikasi desktop lain
harus mengikuti pola yang sama dengan product ID dan engine masing-masing.

## Status implementasi 20 September 2026

Perubahan trial bersama, ID pemakaian per percobaan, dan alur updater sudah
diterapkan dan diverifikasi:

- Gateway production memakai commit `6c6471f` dan sudah sehat setelah deploy
  Coolify. Migrasi hanya menambahkan nilai trial bersama; product ID Mayar,
  harga, checkout URL, webhook, dan data lisensi berbayar tidak diubah.
- Repository release publik adalah `mfahryf/XIX-Vectorizer-release`.
- Release desktop terbaru adalah `v0.1.8` pada repository publik.
- Metadata updater: `https://github.com/mfahryf/XIX-Vectorizer-release/releases/latest/download/latest.json`.
- Installer stabil: `https://github.com/mfahryf/XIX-Vectorizer-release/releases/latest/download/Vectorizer-latest-x64-setup.exe`.
- `latest.json` dan installer stabil sudah dapat diakses tanpa login dengan
  status HTTP `200`; metadata berisi versi `0.1.8` dan signature.
- Saat startup, aplikasi menampilkan pemberitahuan update terlebih dahulu.
  Instalasi hanya berjalan setelah pengguna memilih `Update now`; aplikasi
  kemudian restart setelah pemasangan selesai.
- Pada `v0.1.8`, setiap percobaan pemrosesan yang berhasil mendapat
  `usage_event_id` baru. File yang sama boleh dihitung lagi sebagai percobaan
  baru; pengulangan dengan ID yang sama hanya dipakai untuk retry jaringan dari
  percobaan yang sama.

Verifikasi otomatis terakhir: gateway 131 test, UI desktop 23 test, dan seluruh
test Rust desktop 156 test semuanya lulus. Uji manual yang tersisa untuk setiap
release adalah membuka instalasi versi lama, memastikan modal update terlihat,
memilih update, lalu memastikan state lisensi, identitas perangkat, dan
counter trial tetap ada setelah restart.

## Keputusan produk

- Satu aplikasi desktop memiliki satu produk dan satu lisensi berlangganan
  bulanan.
- Satu lisensi hanya memiliki satu device binding aktif.
- Penggantian perangkat dilakukan admin melalui reset binding; menghapus file
  lokal tidak mereset binding server.
- Trial adalah sepuluh file berhasil total untuk seluruh engine. Semua engine
  berbagi satu penghitung yang sama.
- Lease offline berlaku maksimal 14 hari dan tidak melewati akhir langganan.
- Setelah lease habis, pemrosesan dikunci. Ketika perangkat kembali online dan
  langganan masih aktif, lease baru dapat diterbitkan.

## State dan perilaku

| State | Pemrosesan | Perilaku |
| --- | --- | --- |
| `unactivated` | Belum ada trial | Claim dibuat ketika file pertama akan diproses. |
| `trial` | Selama total trial masih tersisa | Sepuluh file berhasil total lintas engine. |
| `licensed-online` | Semua engine | Lease baru divalidasi server. |
| `licensed-offline` | Semua engine | Lease lokal masih valid tanpa server. |
| `expired-offline` | Terkunci | Minta validasi online; data pengguna tetap ada. |
| `subscription-expired` | Terkunci | Minta pembayaran/perpanjangan. |
| `revoked` | Terkunci | Tampilkan alasan dan bantuan admin. |
| `device-conflict` | Terkunci | Key terikat di perangkat lain. |
| `device-identity-lost` | Terkunci | Buat permintaan recovery admin. |

Penguncian hanya berlaku untuk pemrosesan. Input, output, konfigurasi,
playlist, dan riwayat tidak boleh dihapus atau diubah karena state lisensi.

## Alur client

1. Baca state lokal yang dilindungi DPAPI saat startup.
2. Jika ada identitas perangkat, minta challenge lalu baca status server.
3. Jika lease mendekati atau melewati batas, minta renewal.
4. Jika tidak ada identitas dan pengguna mulai memproses file, buat identitas
   lalu claim trial online.
5. Sebelum setiap batch trial dimulai, periksa satu penghitung total yang
   dipakai bersama oleh semua engine.
6. Kurangi kuota hanya setelah file berhasil dan output tervalidasi.
7. Sinkronkan event usage secara idempoten ketika online.

Tombol `Get License` meminta `GET /v1/desktop/products/xix-vectorizer` ke
gateway lalu membuka `checkout_url` yang dikembalikan katalog admin. Tidak ada
token admin atau rahasia Mayar di desktop. Jika gateway sementara tidak dapat
dihubungi, aplikasi memakai tautan production terakhir sebagai cadangan.

Batch boleh berisi lebih banyak file daripada sisa trial, tetapi worker tidak
boleh memulai file setelah izin terakhir dipakai. File gagal atau dibatalkan
mengembalikan izin dan tidak mengurangi kuota.

## Penyimpanan lokal

Private key Ed25519 dan state lisensi disimpan melalui penyimpanan terlindungi
Windows. `config.json` tidak boleh memuat raw license key, private key,
Mayar secret, atau private signing key gateway. Cache lease boleh dibuat ulang
online pada device yang sama; identitas device yang hilang memerlukan recovery
admin.

Trial server-side, pembayaran, binding, dan audit tidak boleh bergantung pada
file lokal. Counter lokal yang belum tersinkron tidak boleh dikembalikan ke
nilai yang lebih besar hanya karena status server masih tertinggal.

## Kontrak tanda tangan

Semua request state-changing memakai header `X-Desktop-Product` dan body
langsung, bukan envelope `{payload, nonce}`. Urutannya:

1. `POST /v1/desktop/device/challenge` dengan identitas publik.
2. Tanda tangani canonical JSON `{"challenge":"...","device_id":"..."}`.
3. Kirim ulang identitas, challenge, signature, platform, dan versi aplikasi.

Client memverifikasi token trial dan nested lease memakai public key gateway
yang dipin di binary. `XIX_GATEWAY_PUBLIC_KEY_B64` hanya override build untuk
rotasi key yang disengaja. Server tidak pernah mengirim Mayar key atau secret
signing private ke desktop.

## Checklist rilis

- Fresh install belum membuat claim sebelum file pertama.
- Semua engine bersama-sama memiliki tepat sepuluh file berhasil selama trial.
- Batch lebih besar dari sisa trial berhenti tepat setelah kuota habis.
- File gagal atau batal tidak mengurangi kuota. Retry otomatis dengan ID event
  yang sama juga tidak mengurangi kuota kedua kali.
- Dua percobaan sukses atas file yang sama memakai ID berbeda dan masing-masing
  mengurangi satu kuota trial.
- Key valid membuka semua engine dan tidak disimpan sebagai raw key.
- Aktivasi pada device kedua ditolak.
- Lease valid dapat dipakai offline maksimal 14 hari.
- Online setelah lease habis memperbarui lease jika subscription aktif.
- Revoke, subscription expired, identity loss, dan device conflict mengunci
  pemrosesan tanpa menghapus data pengguna.
- Tidak ada secret produksi atau cache lokal di repository.
