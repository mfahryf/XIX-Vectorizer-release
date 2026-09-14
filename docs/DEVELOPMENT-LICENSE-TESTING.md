# Pengujian lisensi saat development

Dokumen ini menjelaskan cara menguji lisensi dari `npm run dev` tanpa membuat
installer. Mode development memakai konfigurasi aplikasi dan public key
verifikasi yang sama dengan client; private key gateway, API key Mayar, dan
secret lain tidak pernah dimasukkan ke aplikasi desktop.

## Pemeriksaan otomatis

Jalankan dari folder repository:

```powershell
npm install
npm run test:ui
cargo test --manifest-path src-tauri/Cargo.toml
```

`npm run test:ui` memeriksa shell UI. `cargo test` memeriksa trial per engine,
perangkat, lease, signature, state penguncian, idempotensi pemakaian, dan
recovery. Public key gateway sudah dipin di client sehingga test tidak
bergantung pada environment lokal.

## Menjalankan aplikasi

```powershell
npm run dev
```

Untuk mengganti public key hanya saat rotasi yang disengaja, set override
build sebelum menjalankan proses dev:

```powershell
$env:XIX_GATEWAY_PUBLIC_KEY_B64 = "<public-key-baru>"
npm run dev
```

Jangan menaruh private key, API key, atau file lisensi nyata di repository.
Hapus override setelah selesai agar pengujian kembali memakai key yang dipin.

## Matriks uji manual

Gunakan satu akun sandbox dan produk `xix-vectorizer-monthly`.

| Skenario | Hasil yang diharapkan |
| --- | --- |
| Fresh start, engine belum dipakai | Belum ada claim sebelum file pertama dimulai. |
| Lima file berhasil pada satu engine | Engine tersebut terkunci; engine lain masih memiliki trial. |
| File gagal, batal, atau retry idempoten | Kuota tidak berkurang dua kali. |
| Aktivasi key valid | Semua engine terbuka dan key tidak disimpan mentah di UI/config. |
| Aktivasi di perangkat kedua | Ditolak sebagai `device-conflict`; tidak membuat binding kedua. |
| Tutup lalu buka kembali aplikasi | State lisensi dan identitas perangkat tetap terbaca. Ini padanan uji logout/login untuk desktop yang memakai license key. |
| Hapus cache lease | Pada device yang sama, lease dapat diminta ulang ketika online. |
| Hapus identitas perangkat | Aplikasi masuk recovery dan tidak membuat identitas pengganti diam-diam. |
| Offline dengan lease valid | Pemrosesan tetap berjalan sampai paling lama 14 hari. |
| Offline setelah lease kedaluwarsa | Pemrosesan terkunci, tetapi file dan data pengguna tetap ada. |
| Online pada hari ke-15, langganan aktif | Validasi berhasil menerbitkan lease baru dan membuka pemrosesan. |
| Reset HWID oleh admin | Device lama ditolak setelah reset; device baru dapat diaktifkan satu kali. |

## Bukti sebelum rilis

- Simpan hasil test otomatis dan versi commit.
- Catat product ID, device test, waktu aktivasi, dan hasil setiap skenario
  manual tanpa menyimpan license key atau secret.
- Pastikan mode sandbox masih digunakan sampai seluruh alur stabil.
- Uji build release terpisah setelah test development lulus.
