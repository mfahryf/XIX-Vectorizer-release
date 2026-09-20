const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");

const source = fs.readFileSync("src-tauri/src/engines/pngtosvg.rs", "utf8");

test("Engine V3 starts Node without opening a Windows console", () => {
  assert.match(source, /std::os::windows::process::CommandExt/);
  assert.match(source, /CREATE_NO_WINDOW/);
  assert.match(source, /fn node_command/);
  assert.match(source, /node_command\("node"\)/);
  assert.match(source, /node_command\(&node\)/);
});
