# XIX Desktop UI Branding and License Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Menyamakan polish UI desktop XIXLabs pada XIX-Vectorizer dan mendokumentasikan aturan yang dapat dipakai aplikasi desktop XIX-* berikutnya.

**Architecture:** Alur lisensi dan identitas produk tetap berada di aplikasi masing-masing. Vectorizer mempertahankan shell Winamp glass yang sudah ada, memakai satu aset merek XIXLabs untuk title bar dan ikon paket Windows, serta menambahkan tombol pembelian di modal lisensi. Standar lintas aplikasi dicatat sebagai kontrak visual dan checklist migrasi, bukan sebagai dependensi runtime bersama.

**Tech Stack:** Tauri 2, HTML/CSS/JavaScript vanilla, aset SVG XIXLabs, generator ikon Tauri, Node.js test runner.

> **Catatan implementasi terbaru (2026-09-20):** Modal lisensi kini menampilkan
> satu counter `TOTAL: n/10` untuk seluruh engine. Rujukan standar UI terbaru
> ada di `docs/XIX-DESKTOP-UI-STANDARD.md`; instruksi lama tentang tiga counter
> terpisah adalah catatan historis.

## Global Constraints

- Aplikasi yang dikerjakan sekarang adalah `XIX-Vectorizer`; aplikasi desktop XIX-* lain hanya menerima dokumentasi standar.
- Aset sumber merek adalah `E:/Playground/XIXLabs/XIX-AnimotionV2/frontend/public/XIX.svg`.
- Tombol lisensi tetap icon-only di toolbar menggunakan `KeyRound`; modal lisensi tetap menjadi tempat status, trial, aktivasi, dan pembelian.
- Trial memiliki 10 file berhasil total lintas engine, satu lisensi per aplikasi,
  satu perangkat, dan lease offline 14 hari.
- Tidak menambahkan secret, API key, atau kredensial Mayar ke frontend maupun binary desktop.
- Link pembelian lisensi berasal dari konfigurasi URL publik aplikasi dan tidak memuat credential.

---

### Task 1: Tambahkan kontrak UI baru ke test

**Files:**
- Modify: `license-modal-structure.test.js`
- Modify: `licensing-ui.test.js` jika diperlukan untuk menjaga kontrak copy trial

**Interfaces:**
- Consumes: struktur HTML dan script frontend saat ini.
- Produces: pemeriksaan otomatis untuk label `Trial Limit:`, tombol `Get License`, identitas title bar, dan pemakaian aset XIXLabs.

- [ ] **Step 1: Tulis assertion yang awalnya gagal**

Tambahkan test yang mencari teks `Trial Limit:`, `Get License`, `by XIXLabs.net`, elemen ikon title bar, dan URL pembelian pada tombol.

- [ ] **Step 2: Jalankan test UI**

Run: `npm run test:ui`

Expected: assertion baru FAIL karena copy, tombol, dan branding belum diterapkan.

- [ ] **Step 3: Pastikan test hanya memeriksa kontrak publik**

Gunakan selector/id yang memang dipakai runtime (`license-trial-v1`, `license-buy`, `titlebar-brand`) dan jangan menguji detail warna atau koordinat piksel.

### Task 2: Polish modal lisensi

**Files:**
- Modify: `src/index.html`
- Modify: `src/main.js`
- Modify: `src/style.css`

**Interfaces:**
- Consumes: `renderLicenseStatus`, `activateLicense`, modal license, dan `window.__TAURI__.shell.open` bila tersedia.
- Produces: copy ringkas `Trial Limit:`, tombol `Get License` di bawah modal, dan pembukaan link pembelian di browser default.

- [ ] **Step 1: Ubah copy counter trial**

Pertahankan heading `Trial Limit:`, tetapi tampilkan satu counter total bersama
sebelum input aktivasi. Jangan membuat counter terpisah per engine.

- [ ] **Step 2: Tambahkan tombol pembelian**

Tambahkan tombol `id="license-buy"` di bawah bantuan lisensi dengan `type="button"`, nama aksesibel `Get License`, dan target URL publik pembelian yang didefinisikan sebagai konstanta frontend. Tombol memanggil `open(url)` dari Tauri shell dan menampilkan status yang jelas jika pembukaan gagal.

- [ ] **Step 3: Rapikan hierarchy modal**

Jaga urutan: badge/status, `Trial Limit:`, counter total, input aktivasi, bantuan, lalu `Get License`. Gunakan style tombol yang sudah dipakai modal agar tidak menciptakan pola visual baru.

- [ ] **Step 4: Jalankan test UI**

Run: `npm run test:ui`

Expected: seluruh test lulus dan tombol pembelian tidak mengubah alur aktivasi.

### Task 3: Terapkan identitas merek pada title bar dan paket Windows

**Files:**
- Add: `src/assets/XIX.svg`
- Modify: `src/index.html`
- Modify: `src-tauri/tauri.conf.json`
- Regenerate: `src-tauri/icons/*` dari aset sumber menggunakan Tauri icon generator

**Interfaces:**
- Consumes: `XIX.svg` dari XIX-AnimotionV2.
- Produces: ikon title bar yang sama dengan ikon aplikasi Windows/installer, serta title `Vectorizer by XIXLabs.net` tanpa awalan `XIX`.

- [ ] **Step 1: Salin aset sumber secara utuh**

Simpan salinan lokal `XIX.svg` di `src/assets/XIX.svg` agar build tidak bergantung pada folder Animotion di luar repo.

- [ ] **Step 2: Tambahkan ikon ke title bar**

Ganti glyph bintang dan teks `XIX-` pada title bar dengan elemen gambar ber-`alt="XIXLabs"` dan teks `VECTORIZER · by XIXLabs.net`. Pertahankan `data-tauri-drag-region` pada area yang dapat menyeret jendela.

- [ ] **Step 3: Atur judul Tauri**

Set `productName` dan `app.windows[0].title` menjadi `Vectorizer by XIXLabs.net`, lalu atur `bundle.icon` menunjuk ke ikon hasil generator di `src-tauri/icons`.

- [ ] **Step 4: Generate ikon platform**

Run: `npx tauri icon src/assets/XIX.svg --output src-tauri/icons`

Expected: file ikon Windows yang diperlukan berubah dan Tauri dapat menemukan ikon tanpa path eksternal.

- [ ] **Step 5: Jalankan build produksi**

Run: `npm run build`

Expected: executable dan installer NSIS berhasil dibuat dengan nama serta ikon baru.

### Task 4: Dokumentasikan standar semua aplikasi desktop XIX-*

**Files:**
- Create: `docs/XIX-DESKTOP-UI-STANDARD.md`
- Modify: `README.md` bila perlu menautkan dokumen standar

**Interfaces:**
- Consumes: hasil implementasi Vectorizer dan kontrak lisensi desktop yang sudah ada.
- Produces: panduan integrasi untuk app desktop baru, termasuk title bar, ikon, modal license, copy, pembelian, state lisensi, dan checklist QA.

- [ ] **Step 1: Dokumentasikan aturan branding**

Tetapkan format title `NamaAplikasi by XIXLabs.net`, larangan awalan `XIX-` pada title jika ikon sudah tampil, aset sumber terlokalisasi di repo, dan penggunaan ikon hasil generator untuk installer.

- [ ] **Step 2: Dokumentasikan aturan modal license**

Tetapkan tombol toolbar icon-only `KeyRound`, label aksesibel, urutan konten modal, `Trial Limit:`, `Get License`, status error, dan dukungan keyboard.

- [ ] **Step 3: Dokumentasikan kontrak lisensi**

Catat trial 10 file total lintas engine, satu aplikasi satu lisensi, satu perangkat,
reset HWID melalui admin, lease 14 hari, dan penguncian pemrosesan setelah
validasi kedaluwarsa.

- [ ] **Step 4: Dokumentasikan checklist rilis**

Masukkan pemeriksaan title bar, installer, icon cache, link pembelian sandbox/production, aktivasi, offline lease, penghapusan file lisensi, reset device, dan regresi pemrosesan.

### Task 5: Verifikasi dan kirim perubahan

**Files:**
- Verify: seluruh file yang berubah pada Task 1–4

**Interfaces:**
- Consumes: test UI, konfigurasi Tauri, ikon hasil generate, dan dokumen standar.
- Produces: commit siap rilis pada branch `main`.

- [ ] **Step 1: Jalankan test UI**

Run: `npm run test:ui`

Expected: seluruh test PASS.

- [ ] **Step 2: Periksa format dan status Git**

Run: `git diff --check` dan `git status --short`

Expected: tidak ada whitespace error dan hanya file yang terkait task ini yang berubah.

- [ ] **Step 3: Build installer**

Run: `npm run build`

Expected: build executable dan installer NSIS PASS.

- [ ] **Step 4: Commit dan push**

```bash
git add src src-tauri/tauri.conf.json docs README.md license-modal-structure.test.js licensing-ui.test.js
git commit -m "feat: polish xix desktop branding and license ui"
git push origin main
```

Expected: commit tersedia di remote `main`, tanpa secret atau file kredensial ikut terkirim.

---

## Self-review checklist

- [ ] Semua permintaan polish untuk Vectorizer punya task implementasi dan test.
- [ ] Aplikasi desktop lain hanya menerima dokumentasi, tidak disentuh tanpa permintaan terpisah.
- [ ] Link pembelian bersifat publik dan bisa diganti dari sandbox ke production tanpa mengubah kontrak lisensi.
- [ ] Aset ikon lokal membuat build reproducible dan tidak bergantung pada path mesin pengembang.
- [ ] Test, diff check, dan build dijalankan sebelum laporan akhir.
