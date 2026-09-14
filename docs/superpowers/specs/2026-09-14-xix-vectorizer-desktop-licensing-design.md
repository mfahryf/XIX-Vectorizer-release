# Desain Lisensi Desktop XIX-Vectorizer

**Tanggal:** 2026-09-14  
**Status:** Disetujui untuk perencanaan implementasi  
**Produk:** `xix-vectorizer`  
**Cakupan:** Aplikasi desktop XIXLabs

## Tujuan

Menambahkan sistem lisensi terpusat untuk XIX-Vectorizer tanpa mewajibkan
Google Auth. Aplikasi memakai trial terbatas, lalu memerlukan license key
dengan langganan bulanan. Lisensi terikat pada satu perangkat dan dapat
dipulihkan admin saat pengguna mengganti perangkat atau kehilangan data lokal.

## Keputusan bisnis

- Desktop memakai license key, bukan Google Auth.
- Satu aplikasi memerlukan satu pembayaran dan satu lisensi.
- Lisensi XIX-Vectorizer tidak membuka aplikasi desktop lainnya.
- Semua lisensi desktop memakai langganan bulanan.
- Satu license key hanya aktif pada satu perangkat.
- Penggantian perangkat dilakukan melalui reset device binding oleh admin.
- Trial dimulai otomatis saat file pertama diproses.
- Setiap engine memiliki trial lima file berhasil secara terpisah: `vectorize-v1`,
  `vectorize-v2`, dan `pngtosvg` (nama tampilan: Vectorize V3).
- Setelah satu engine mencapai lima file, hanya engine itu yang terkunci.
- Setelah semua engine menghabiskan trial, seluruh pemrosesan memerlukan lisensi.
- Lease lokal berlaku maksimal 14 hari sejak validasi terakhir.
- Pembayaran terkonfirmasi membuat atau memperpanjang lisensi secara otomatis,
  menampilkan license key, dan mengirimkannya ke email pembeli.
- Penguncian tidak menghapus file input, hasil, atau pengaturan lokal.

## Arsitektur dan tanggung jawab

`XIX-Payment-Gateway` menjadi sumber kebenaran pembayaran dan lisensi.
`XIX-Vectorizer` hanya meminta status dan bukti lisensi. Desktop tidak menyimpan
kredensial Mayar dan tidak menghubungi Mayar secara langsung.

- `XIX-Payment-Gateway` menerima konfirmasi pembayaran, membuat lisensi,
  memperpanjang lisensi, menerbitkan lease, dan mencatat pemakaian.
- `XIXLabs-admin` mengatur reset perangkat, suspend, revoke, penerbitan ulang,
  dan pemeriksaan audit.
- `XIX-Vectorizer` menampilkan status, memulai trial, menghitung pemakaian, dan
  menyimpan lease secara aman.
- `XIX-Auth` tidak menjadi prasyarat aktivasi desktop.

Kontrak ini harus dapat dipakai ulang oleh BGRemover, SVGConverter, Upscaler,
dan aplikasi desktop XIXLabs berikutnya.

## Istilah domain

### Produk

Identitas aplikasi yang dijual. Nilai pilot adalah `xix-vectorizer`. Engine
adalah bagian dari produk, bukan produk yang dibeli terpisah.

### License key

Kode yang diberikan setelah pembayaran terkonfirmasi. Key hanya mengaktifkan
produk yang dibeli.

### Device binding

Hubungan antara satu lisensi dan satu perangkat. Aplikasi membuat kunci
identitas perangkat dan menyimpannya melalui penyimpanan aman Windows. Sinyal
perangkat boleh menjadi pemeriksaan tambahan, tetapi HWID mentah tidak menjadi
satu-satunya identitas.

### Lease lisensi

Bukti lisensi bertanda tangan server yang disimpan lokal. Lease berlaku paling
lama 14 hari dan tidak boleh melewati tanggal berakhir lisensi bulanan.

### Trial usage

Jumlah file berhasil yang sudah diproses oleh setiap engine selama trial.
Status utamanya disimpan server-side agar penghapusan file lokal tidak mengulang
trial.

## State aplikasi

State aplikasi merupakan gabungan status lisensi produk dan kuota tiap engine.

| State | Kondisi | Perilaku |
|---|---|---|
| `unactivated` | Belum ada trial atau lisensi | Tampilkan aktivasi; trial dibuat ketika file pertama diproses |
| `trial-active` | Sedikitnya satu engine masih memiliki kuota | Engine yang tersedia dapat memproses |
| `licensed-online` | Lisensi aktif dan lease valid | Semua engine terbuka |
| `licensed-offline` | Server tidak dihubungi, lease belum habis | Semua engine tetap terbuka |
| `engine-trial-exhausted` | Satu engine sudah mencapai lima file | Engine tersebut terkunci; engine lain mengikuti kuotanya |
| `expired-offline` | Lease lokal habis | Semua pemrosesan dikunci sampai validasi online berhasil |
| `subscription-expired` | Langganan bulanan berakhir | Semua pemrosesan dikunci sampai diperpanjang |
| `revoked` | Lisensi dicabut admin | Semua pemrosesan dikunci dan alasan ditampilkan |
| `device-conflict` | Key terikat pada perangkat lain | Aktivasi ditolak dan diarahkan ke pemulihan admin |
| `server-unavailable` | Server tidak dapat dihubungi | Lease valid tetap dipakai; lease habis berarti terkunci |

Halaman aktivasi, bantuan, pengaturan, dan akses ke file lama tetap tersedia
ketika pemrosesan terkunci.

## Alur pembayaran dan penerbitan lisensi

1. Pembeli membayar produk `xix-vectorizer` melalui Mayar.
2. Payment gateway menerima notifikasi pembayaran terkonfirmasi.
3. Notifikasi diproses idempoten agar pengulangan tidak membuat lisensi ganda.
4. Sistem membuat lisensi baru atau memperpanjang lisensi yang sesuai.
5. License key ditampilkan pada halaman sukses pembayaran.
6. License key dikirim ke email transaksi.
7. Hubungan pembayaran, produk, lisensi, dan masa berlaku dicatat.

Jika email gagal, lisensi tetap aktif. Admin dapat melakukan rotasi key tanpa
menghapus bukti pembayaran.

## Alur trial

1. Ketika pengguna memulai pemrosesan file pertama, aplikasi membuat identitas
   perangkat aman.
2. Aplikasi meminta trial produk ke payment gateway.
3. Server memastikan perangkat belum pernah memperoleh trial produk tersebut.
4. Server mengembalikan trial dengan kuota lima file untuk setiap engine.
5. Aplikasi memproses file dan memperbarui penghitung engine yang dipakai.

Satu file sumber yang menghasilkan output valid mengurangi satu kuota. Satu
batch berisi sepuluh file mengurangi sepuluh kuota. File gagal, dibatalkan,
atau tidak menghasilkan output valid tidak dihitung. Retry atas file yang sama
harus idempoten.

Trial tidak dimulai hanya karena aplikasi dibuka atau dipasang. Reinstall dan
penghapusan cache tidak boleh membuat trial baru pada perangkat yang sama.

Claim trial pertama memerlukan koneksi online. Setelah trial berhasil dibuat,
aplikasi boleh memakai kuota yang sudah diterbitkan ketika offline dengan token
trial bertanda tangan dan penghitung lokal yang disimpan aman. Pemakaian offline
disinkronkan ke server pada koneksi berikutnya. Server tetap menjadi sumber
kebenaran untuk claim trial dan akan menolak atau menandai penghitung yang
tidak konsisten; penghapusan token lokal hanya mengunci pemrosesan sampai
aplikasi terhubung kembali.

## Alur aktivasi dan validasi

1. Pengguna memasukkan license key pada halaman aktivasi.
2. Aplikasi meminta challenge sekali-pakai, lalu mengirim key, produk, versi
   aplikasi, identitas perangkat, challenge, dan tanda tangan melalui HTTPS.
3. Server memeriksa key, masa langganan, status revoke, dan device binding.
4. Jika key belum terikat, server mengikatnya ke perangkat.
5. Jika key sudah terikat pada perangkat yang sama, server menerbitkan lease baru.
6. Jika key terikat pada perangkat lain, server mengembalikan `device-conflict`.
7. Lease yang berhasil diterima disimpan aman dan semua engine dibuka.

Aplikasi mencoba memperbarui lease ketika dibuka dan ketika lease mendekati
masa berakhir. Hari ke-15 setelah lease habis tidak otomatis berarti lisensi
hangus: jika server mengonfirmasi langganan masih aktif, lease baru diterbitkan
dan pemrosesan dibuka kembali.

Jika langganan berakhir, lease lokal tidak boleh memperpanjang akses melewati
tanggal berakhir langganan.

## Penghapusan data lokal dan penggantian perangkat

- Jika hanya cache lease dihapus, aplikasi meminta validasi online dan menerima
  lease baru untuk perangkat yang sama.
- Jika identitas perangkat ikut dihapus, instalasi dianggap sebagai identitas
  baru dan memerlukan pemulihan admin.
- Penghapusan data lokal tidak menghapus trial usage, pembayaran, atau binding
  di server.
- Penggantian perangkat memerlukan verifikasi license key atau data transaksi,
  lalu admin menonaktifkan binding lama dan mencatat alasan reset.
- Reset tidak menghapus masa langganan atau riwayat pembayaran.

## Kontrak layanan pusat

Nama host final mengikuti deployment pusat. Rute logis yang diperlukan:

- `POST /v1/desktop/trial/claim`: membuat atau mengambil trial produk.
- `POST /v1/desktop/license/activate`: memvalidasi key dan membuat binding.
- `GET /v1/desktop/license/status`: mengambil status lisensi, trial, dan lease.
- `POST /v1/desktop/license/renew`: memperbarui lease saat lisensi masih aktif.
- `POST /v1/desktop/usage/record`: mencatat file berhasil secara idempoten.

Setiap permintaan desktop memuat product header, versi aplikasi, identitas
perangkat, challenge sekali-pakai, dan tanda tangan. Aktivasi juga memuat
license key; renewal tidak memuat raw key. Server tidak pernah mengirim
rahasia Mayar ke desktop.

Respons status minimal memuat status lisensi, tanggal berakhir langganan,
tanggal berakhir lease, status binding, sisa trial per engine, alasan lock,
waktu server, dan tanda tangan server.

Semua endpoint aktivasi, renewal, pencatatan penggunaan, dan webhook pembayaran
harus idempoten.

## Data pusat

Skema final mengikuti database pusat, tetapi harus mencakup tanggung jawab berikut:

- `license_products`: produk dan engine yang dimiliki.
- `licenses`: status, produk, pemilik pembelian, dan masa berlaku.
- `license_keys`: sidik key, status rotasi, dan metadata penerbitan.
- `devices`: identitas perangkat publik dan status binding.
- `license_activations`: hubungan lisensi-perangkat dan riwayat aktivasi.
- `license_entitlements`: hak akses produk atau engine.
- `trial_usage`: penghitung file berhasil per engine dan perangkat.
- `license_leases`: lease yang diterbitkan dan masa berlakunya.
- `license_events`: audit pembayaran, aktivasi, renewal, reset, suspend, revoke,
  dan pemulihan.
- `payment_license_links`: hubungan pembayaran Mayar dengan lisensi.

License key mentah tidak disimpan sebagai teks biasa setelah penerbitan. Jika
key hilang, admin melakukan rotasi key dan mengirim key pengganti.

## Perubahan pada Vectorizer

Implementasi dipusatkan pada batas lisensi dan tidak mengubah algoritma engine:

- Modul Rust untuk identitas perangkat, penyimpanan DPAPI, tanda tangan
  permintaan, validasi lease, dan pemeriksaan waktu server.
- Klien HTTPS yang hanya menghubungi payment gateway pusat.
- State machine lisensi terpisah dari batch processor.
- Halaman aktivasi dan status lisensi pada UI Tauri.
- Penghitung trial V1, V2, dan V3.
- Gate sebelum batch dimulai dan pemeriksaan sebelum setiap file trial.
- Pesan yang membedakan lease habis, langganan berakhir, revoke, dan konflik
  perangkat.
- Tidak ada kredensial Mayar, client secret, atau rahasia global dalam binary.

## Keamanan

- Komunikasi memakai HTTPS.
- Lease ditandatangani server; desktop hanya menyimpan public key verifikasi.
- Kunci privat perangkat disimpan melalui penyimpanan aman Windows.
- License key tidak dicatat dalam log.
- Trial, aktivasi, reset, dan revoke memiliki audit log.
- Claim trial, aktivasi, dan pencatatan penggunaan dibatasi frekuensinya.
- Perubahan jam, penghapusan cache, dan reinstall tidak memperpanjang entitlement.
- Reset perangkat tidak dilakukan otomatis oleh desktop.

## Pengalaman pengguna

Halaman lisensi menampilkan nama produk, status perangkat, tombol aktivasi,
kuota trial setiap engine seperti `V1 3/5`, masa offline yang tersisa, alasan
penguncian, dan kontak bantuan. HWID mentah tidak ditampilkan. Untuk dukungan,
aplikasi membuat kode permintaan pemulihan yang aman.

## Pengujian penerimaan

Implementasi dianggap selesai jika:

1. Trial otomatis dibuat pada pemrosesan file pertama.
2. Setiap engine dapat memproses tepat lima file berhasil.
3. Engine yang habis kuota terkunci tanpa mengunci engine lain.
4. File gagal, batal, dan retry idempoten mengikuti aturan kuota.
5. Trial tidak kembali setelah reinstall atau penghapusan cache.
6. Key valid membuka semua engine.
7. Key yang sama ditolak pada perangkat kedua.
8. Reset admin memungkinkan perangkat baru diaktivasi.
9. Lease valid membuat aplikasi berjalan tanpa koneksi.
10. Lease habis mengunci pemrosesan.
11. Validasi online setelah lease habis membuka kembali lisensi yang masih aktif.
12. Lisensi kedaluwarsa, dicabut, atau pembayaran gagal tetap terkunci.
13. Cache lease yang dihapus dapat dipulihkan pada perangkat yang sama.
14. Identitas perangkat yang dihapus memerlukan pemulihan admin.
15. Webhook Mayar berulang hanya membuat satu lisensi atau satu perpanjangan.
16. File dan pengaturan pengguna tidak hilang pada semua state lisensi.

Pengujian terdiri dari unit test state machine dan penghitung trial, integration
test layanan pusat, serta pengujian manual Tauri untuk aktivasi, offline,
pemulihan, pembayaran, dan konflik perangkat.

## Urutan implementasi

1. Tetapkan kontrak layanan dan skema data di payment gateway pusat.
2. Tambahkan webhook Mayar idempoten dan penerbitan key.
3. Tambahkan endpoint trial, aktivasi, status, renewal, dan usage record.
4. Buat modul lisensi aman di Vectorizer.
5. Tambahkan tampilan aktivasi, status, dan penghitung trial.
6. Hubungkan gate lisensi dengan batch processor.
7. Tambahkan reset, revoke, dan audit di `XIXLabs-admin`.
8. Jalankan seluruh pengujian penerimaan sebelum rilis.

Harga, pajak, dan metode pembayaran tetap dikelola oleh Mayar. Desain visual
dashboard admin dibahas dalam dokumen terpisah untuk repository `XIXLabs-admin`.
