# XIX-Vectorizer

Aplikasi desktop untuk mengubah gambar menjadi SVG.

Engine yang tersedia:

- Vectorize V1 — layanan SVG AI
- Vectorize V2 — layanan svg.new
- Vectorize V3 — pemrosesan lokal pngtosvg

Jalankan mode development dari folder ini dengan:

```powershell
npm install
npm run dev
```

## Lisensi desktop

XIX-Vectorizer memakai lisensi bulanan per aplikasi. Pembelian Vectorizer
hanya membuka Vectorizer; lisensi tidak berlaku otomatis untuk aplikasi XIX-
lainnya.

### Trial

- Trial dimulai ketika file pertama benar-benar diproses.
- Setiap engine memiliki kuota lima file berhasil: `vectorize-v1`,
  `vectorize-v2`, dan `pngtosvg` (ditampilkan sebagai Vectorize V3).
- File gagal, dibatalkan, dan retry yang sama tidak mengurangi kuota.
- Jika satu engine habis, hanya engine itu yang terkunci. Engine lain tetap
  dapat dipakai sampai kuotanya habis.
- Klaim trial pertama memerlukan koneksi internet. Setelah diklaim, counter
  lokal dapat dipakai saat offline.

### Aktivasi dan perangkat

1. Selesaikan pembayaran bulanan melalui checkout Software License Mayar yang tercantum di bagian pilot dokumen ini.
2. Salin kode lisensi yang ditampilkan atau dikirim melalui email Mayar.
3. Buka panel `LICENSE`, masukkan key, lalu pilih `ACTIVATE`.
4. Satu lisensi hanya memiliki satu device binding aktif di gateway XIXLabs.
5. Penggantian komputer dilakukan oleh admin dengan mereset binding perangkat
   di layanan lisensi. Menghapus file lokal tidak mereset binding server.

Private key perangkat disimpan memakai Windows DPAPI. Lease dan usage ledger
disimpan terpisah dari `config.json`; file pengaturan UI tidak memuat license
key, private key, atau secret pembayaran. Cache lease yang terhapus pada
perangkat yang sama dapat diterbitkan ulang setelah online. Jika identitas
perangkat juga hilang, aplikasi menampilkan state pemulihan dan admin perlu
menangani reset perangkat.

### Offline dan state penguncian

Lease lokal berlaku paling lama 14 hari dan tidak boleh melewati akhir
langganan. Saat lease masih valid, proses tetap dapat berjalan offline. Setelah
lease habis, proses dikunci sampai validasi online berhasil. Jika pada hari ke
15 perangkat kembali online dan langganan masih aktif, lease baru dapat
diterbitkan. Status `subscription-expired`, `revoked`, atau `device-conflict`
juga mengunci pemrosesan dan memberi petunjuk pemulihan.

Penguncian hanya berlaku untuk pemrosesan. File input, output lama, playlist,
pengaturan, riwayat, dan panel bantuan tidak dihapus.

### Kontrak layanan pusat

Client desktop hanya berkomunikasi dengan `XIX-Payment-Gateway` melalui
HTTPS. Endpoint yang digunakan:

- `POST /v1/desktop/trial/claim`
- `POST /v1/desktop/license/activate`
- `GET /v1/desktop/license/status`
- `POST /v1/desktop/license/renew`
- `POST /v1/desktop/usage/record`

Setiap request memakai challenge sekali-pakai dari gateway. Signature mencakup
canonical JSON `{"challenge":"...","device_id":"..."}`; tidak ada envelope
`payload/nonce`. Desktop tidak menyimpan API key Mayar, Google Auth secret,
atau private key penandatangan lease gateway. Binary membawa public verification
key gateway yang dipin di source. `XIX_GATEWAY_PUBLIC_KEY_B64` hanya dipakai
sebagai override saat rotasi key yang direncanakan; nilainya adalah public key,
bukan secret. `XIX_GATEWAY_URL` dapat dipakai sebagai override build-time untuk
gateway lokal saat development; release tetap memakai gateway production.

Format lengkap request, response, lease, token trial, dan error ada di
[`docs/DESKTOP-LICENSING-CONTRACT.md`](docs/DESKTOP-LICENSING-CONTRACT.md).
Cara menjalankan dan memeriksa alur lisensi tanpa membuat installer ada di
[`docs/DEVELOPMENT-LICENSE-TESTING.md`](docs/DEVELOPMENT-LICENSE-TESTING.md).
Aturan tampilan desktop lintas aplikasi XIXLabs ada di
[`docs/XIX-DESKTOP-UI-STANDARD.md`](docs/XIX-DESKTOP-UI-STANDARD.md).

### Checklist verifikasi rilis

- [ ] Fresh install menampilkan tiga counter trial dan belum membuat klaim
      sebelum file pertama diproses.
- [ ] File pertama per engine meminta klaim online dan kuota mulai dari lima.
- [ ] Lima file berhasil mengunci hanya engine tersebut.
- [ ] File gagal, cancel, dan retry idempoten tidak mengurangi kuota tambahan.
- [ ] Aktivasi key valid membuka semua engine dan mengosongkan input key dari
      layar.
- [ ] Aktivasi lisensi yang sudah terikat ke device lain menampilkan bantuan
      admin, bukan membuat binding kedua.
- [ ] Cache lease valid dapat dipakai offline sampai 14 hari.
- [ ] Lease expired mengunci proses tetapi tidak menghapus data pengguna.
- [ ] Online setelah expired memperbarui lease bila langganan masih aktif.
- [ ] Menghapus cache lease meminta validasi online pada device yang sama.
- [ ] Kehilangan identitas device menampilkan state recovery.
- [ ] Tidak ada secret produksi, cache lease, identitas device, ledger, atau
      database lokal yang ikut Git.

Kode lisensi yang dimasukkan adalah `licenseCode` dari Mayar, bukan key yang
dibuat gateway. Gateway memisahkan status provider Mayar dari status langganan
bulanan XIXLabs; status provider dapat tetap aktif ketika akses bulanan sudah
berakhir.

Untuk pilot, pakai pasangan yang sedang aktif di katalog gateway: Mayar product
ID `c0bf5eb7-2cb2-4caf-9768-51f07dd54b22` dengan checkout
`https://xixlabs.myr.lat/pl/xix-vectorizer-monthly-license`. Pasangan itu milik akun
sandbox, dan katalog gateway adalah sumber kebenarannya, bukan halaman ini. Saat
pindah ke production, ganti Mayar product ID dan checkout bersamaan mengikuti
panduan di `XIX-Payment-Gateway/docs/PRODUCTION-CUTOVER.md`. Lakukan satu
transaksi test, ambil kode dari halaman/email Mayar, lalu aktivasi pada satu
komputer. Jangan memakai secret produksi di test lokal atau repository.
