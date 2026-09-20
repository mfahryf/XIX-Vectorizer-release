const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");

const config = JSON.parse(fs.readFileSync("src-tauri/tauri.conf.json", "utf8"));
const packageJson = JSON.parse(fs.readFileSync("package.json", "utf8"));
const indexSource = fs.readFileSync("src/index.html", "utf8");

test("Tauri window title and bundle use the XIX desktop branding contract", () => {
  assert.equal(config.productName, "Vectorizer");
  assert.equal(config.app.windows[0].title, "Vectorizer by XIXLabs.net");
  assert.ok(Array.isArray(config.bundle.icon));
  assert.ok(config.bundle.icon.includes("icons/icon.ico"));
});

test("title bar displays the current desktop version", () => {
  assert.match(indexSource, /id="titlebar-version"/);
  assert.match(indexSource, new RegExp(`>v${packageJson.version}<`));
});
