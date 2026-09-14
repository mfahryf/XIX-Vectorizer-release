const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");

const html = fs.readFileSync("src/index.html", "utf8");
const script = fs.readFileSync("src/main.js", "utf8");

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
