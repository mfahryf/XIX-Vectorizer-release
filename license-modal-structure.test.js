const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");

const html = fs.readFileSync("src/index.html", "utf8");
const script = fs.readFileSync("src/main.js", "utf8");
const style = fs.readFileSync("src/style.css", "utf8");

test("license controls live in a dedicated KeyRound modal", () => {
  const modalStart = html.indexOf('id="license-modal-overlay"');
  const licensePanel = html.indexOf('id="license-panel"');
  const settingsStart = html.indexOf('id="modal-overlay"');

  assert.notEqual(modalStart, -1);
  assert.ok(licensePanel > modalStart);
  assert.ok(modalStart > settingsStart);
  assert.match(html, /id="btn-license"[^>]*aria-label="Lisensi"/);
  assert.match(html, /id="btn-license"[\s\S]*?key-round/);
});

test("license modal has an accessible close and escape handling", () => {
  assert.match(html, /id="license-modal"[^>]*role="dialog"[^>]*aria-modal="true"/);
  assert.match(html, /id="license-close"[^>]*aria-label="Tutup lisensi"/);
  assert.match(script, /function openLicenseModal\(\)/);
  assert.match(script, /function closeLicenseModal\(\)/);
  assert.match(script, /event\.key === "Escape"/);
});

test("license modal exposes the trial limit and public purchase action", () => {
  assert.match(html, /class="license-trial-label">Trial Limit:<\/div>/);
  assert.match(html, /id="license-buy"[^>]*aria-label="Get License"[^>]*>Get License<\/button>/);
  assert.match(html, /id="license-activate"[^>]*>ACTIVATE<\/button>/);
  assert.match(script, /LICENSE_PURCHASE_URL/);
  assert.match(script, /openUrl\(LICENSE_PURCHASE_URL\)/);
  assert.doesNotMatch(script, /Belum diaktifkan\./);
  assert.match(script, /focusable = \[[\s\S]*?\$\("license-buy"\)/);
  assert.match(script, /licenseBuy\.disabled = true/);
  assert.match(script, /licenseBuy\.setAttribute\("aria-busy", "true"\)/);
  assert.match(script, /Membuka halaman lisensi…/);
  assert.match(style, /body #license-modal \.modal-title[\s\S]*?height: 30px;[\s\S]*?font-size: 9px;/);
  assert.doesNotMatch(style, /body #license-modal \.license-trial-label \{[^}]*text-transform:\s*uppercase;/);
});

test("title bar uses the local XIXLabs mark and application byline", () => {
  assert.match(html, /id="titlebar-brand"[^>]*src="assets\/XIX\.svg"/);
  assert.match(html, /id="titlebar-label"[^>]*>VECTORIZER<\/span>/);
  assert.match(html, /id="titlebar-byline"[^>]*>by XIXLabs\.net<\/span>/);
  assert.doesNotMatch(html, /★ XIX-VECTORIZER/);
});
