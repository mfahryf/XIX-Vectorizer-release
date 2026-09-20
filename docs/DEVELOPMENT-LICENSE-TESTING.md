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

`npm run test:ui` memeriksa shell UI. `cargo test` memeriksa trial total lintas engine,
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

Untuk menguji gateway lokal, URL juga dapat dioverride saat build dev:

```powershell
$env:XIX_GATEWAY_URL = "http://127.0.0.1:8000"
npm run dev
```

Override ini hanya masuk ke build lokal; jangan menggunakannya untuk release.

Jangan menaruh private key, API key, atau file lisensi nyata di repository.
Hapus override setelah selesai agar pengujian kembali memakai key yang dipin.

## Matriks uji manual

Gunakan satu akun sandbox dan produk Software License Mayar
`c0bf5eb7-2cb2-4caf-9768-51f07dd54b22` pada akun sandbox, dengan
checkout `https://xixlabs.myr.lat/pl/xix-vectorizer-monthly-license`. Pasangan ini
dibaca dari katalog gateway lewat console admin dan berubah bersama saat cutover
ke production, jadi jangan disalin ke dokumen lain.

| Skenario | Hasil yang diharapkan |
| --- | --- |
| Fresh start, engine belum dipakai | Belum ada claim sebelum file pertama dimulai. |
| Sepuluh file berhasil lintas engine | Trial habis untuk semua engine; lisensi diperlukan untuk melanjutkan. |
| Lima file berhasil pada satu engine | Trial total berkurang lima; engine lain memakai sisa penghitung yang sama. |
| File gagal atau batal | Kuota tidak berkurang. |
| Retry otomatis dari percobaan yang sama | ID event sama; kuota tidak berkurang dua kali. |
| File yang sama diproses lagi | ID event baru; kuota berkurang satu lagi sampai total 10. |
| Aktivasi kode Mayar valid | Semua engine terbuka dan kode tidak disimpan mentah di UI/config. |
| Aktivasi di perangkat kedua | Ditolak sebagai `device-conflict`; tidak membuat binding kedua. |
| Tutup lalu buka kembali aplikasi | State lisensi dan identitas perangkat tetap terbaca. Ini padanan uji logout/login untuk desktop yang memakai kode Mayar. |
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
- Uji kasus pembayaran 5 September dan aktivasi 12 September: periode harus
  tetap berakhir 5 Oktober, bukan 12 Oktober.
- Uji kode Mayar tetap `ACTIVE` tetapi periode XIXLabs sudah berakhir: akses
  harus terkunci dengan pesan perpanjangan, bukan pesan kode Mayar kadaluarsa.
- Uji build release terpisah setelah test development lulus.

## Bukti rilis terbaru

Pada 20 September 2026, perbaikan ID pemakaian per percobaan, trial bersama,
dan updater dirilis sebagai `v0.1.10` di repository publik
`mfahryf/XIX-Vectorizer-release`. Gateway terkait sudah dideploy dari commit
`6c6471f` dan health production tetap berhasil.

Hasil pengujian otomatis pada release tersebut:

- `npm run test:ui`: 23 lulus;
- `cargo test --manifest-path src-tauri/Cargo.toml --lib`: 156 lulus;
- gateway `python -m pytest -q`: 131 lulus.

Asset updater diverifikasi tanpa login: `latest.json` dan
`Vectorizer-latest-x64-setup.exe` sama-sama mengembalikan HTTP `200`. Uji
manual upgrade dari instalasi `v0.1.9` ke `v0.1.10` tetap menjadi pemeriksaan
akhir sebelum release dinyatakan teruji penuh pada perangkat pengguna.
