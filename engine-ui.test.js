const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const { formatEngineLabel, sortEngines } = require("./src/engine-ui.js");
const html = fs.readFileSync("src/index.html", "utf8");
const script = fs.readFileSync("src/main.js", "utf8");
const style = fs.readFileSync("src/style.css", "utf8");

const engines = [
  { id: "vectorize-v2", name: "Vectorize V2" },
  { id: "pngtosvg", name: "Vectorize V3" },
  { id: "vectorize-v1", name: "Vectorize V1" },
];

test("engine options are ordered V1, V2, then V3", () => {
  assert.deepEqual(
    sortEngines(engines).map((engine) => engine.id),
    ["vectorize-v1", "vectorize-v2", "pngtosvg"],
  );
});

test("engine labels identify online and offline processing with superscript text", () => {
  assert.equal(formatEngineLabel(engines[2]), "Vectorize V1 ⁽ᴼⁿˡᶦⁿᵉ⁾");
  assert.equal(formatEngineLabel(engines[1]), "Vectorize V3 ⁽ᴼᶠᶠˡᶦⁿᵉ⁾");
});

test("the frontend uses the shared engine presentation helpers", () => {
  assert.match(html, /<script src="engine-ui\.js"><\/script>[\s\S]*?<script src="main\.js"><\/script>/);
  assert.match(script, /XixEngineUI\.sortEngines/);
  assert.match(script, /XixEngineUI\.formatEngineLabel/);
});

test("the long engine labels stay readable inside the compact dropdown", () => {
  assert.match(style, /\.dd-head \.dd-label[\s\S]*?min-width: 0;[\s\S]*?text-overflow: ellipsis;/);
  assert.match(style, /body #dd-engine \.dd-list[\s\S]*?width: max-content;[\s\S]*?right: auto;/);
  assert.match(style, /body #dd-engine \.dd-opt[\s\S]*?white-space: nowrap;/);
});
