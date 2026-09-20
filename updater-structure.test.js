const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname);
const config = JSON.parse(fs.readFileSync(path.join(root, "src-tauri", "tauri.conf.json"), "utf8"));
const mainSource = fs.readFileSync(path.join(root, "src", "main.js"), "utf8");
const htmlSource = fs.readFileSync(path.join(root, "src", "index.html"), "utf8");
const rustSource = fs.readFileSync(path.join(root, "src-tauri", "src", "lib.rs"), "utf8");

test("desktop bundle produces signed updater artifacts for the public release repository", () => {
  assert.equal(config.bundle.createUpdaterArtifacts, true);
  assert.equal(typeof config.plugins?.updater?.pubkey, "string");
  assert.ok(config.plugins.updater.pubkey.trim().length > 20);
  assert.deepEqual(config.plugins.updater.endpoints, [
    "https://github.com/mfahryf/XIX-Vectorizer-release/releases/latest/download/latest.json",
  ]);
});

test("frontend shows an in-app update notice before installing", () => {
  assert.match(mainSource, /updater\?\.check/);
  assert.match(mainSource, /showUpdatePrompt/);
  assert.match(mainSource, /downloadAndInstall/);
  assert.match(mainSource, /process\?\.relaunch/);
  assert.match(mainSource, /plugin:updater\|check/);
  assert.match(mainSource, /plugin:updater\|download_and_install/);
  assert.match(mainSource, /plugin:process\|restart/);
  assert.match(mainSource, /void checkForUpdates\(\);/);
  assert.doesNotMatch(mainSource, /window\.confirm/);
  assert.match(mainSource, /restartAfterInstall: true/);
  assert.match(htmlSource, /id="update-modal-overlay"/);
  assert.match(htmlSource, /id="update-install"/);
  assert.match(htmlSource, /id="update-later"/);
});

test("native updater and process plugins are registered", () => {
  assert.match(rustSource, /plugin\(tauri_plugin_process::init\(\)\)/);
  assert.match(rustSource, /plugin\(tauri_plugin_updater::Builder::new\(\)\.build\(\)\)/);
});

test("frontend refreshes the license status after a batch completes", () => {
  assert.match(mainSource, /listen\("batch:\/\/done"[\s\S]*?void loadLicenseStatus\(\);/);
});
