// Desktop runner for the pngtosvg offline vectorizer.
//
// The Tauri engine shells out to this script with bundled Node:
//   node runner.js <input> <output.svg> <max-dim> <colors>
//
// It decodes the image to RGBA, runs the pngtosvg worker (worker.js, same
// folder), and writes the RAW svg to <output.svg>. Fit/sync (~25MP) happens
// in the Rust engine (shared fit.rs) so the output matches every other
// engine.
//
// Decoder: sharp when available (dev, webp support); otherwise pngjs +
// jpeg-js (pure JS, bundled — png/jpg only). Downscale beyond <max-dim> via
// simple nearest-neighbour sampling.

const fs = require('fs');
const path = require('path');
const { detectImageType } = require(path.join(__dirname, 'detect.js'));

const [, , inPath, outPath, maxDimArg, colorsArg] = process.argv;
const MAX_DIM = parseInt(maxDimArg || '2000', 10) || 2000;
// 0 (or missing) = auto-detect image type (sketch/logo/photo) like the website
const COLORS_ARG = parseInt(colorsArg || '0', 10);
const COLORS = Number.isFinite(COLORS_ARG) && COLORS_ARG > 0 ? COLORS_ARG : 0;

function runWorker(image, settings) {
  return new Promise((resolve, reject) => {
    const workerSrc = fs.readFileSync(path.join(__dirname, 'worker.js'), 'utf8');
    let done = false;
    global.self = {
      postMessage: (msg) => {
        if (msg.type === 'done') {
          done = true;
          resolve(msg.result);
        } else if (msg.type === 'error') {
          reject(new Error(msg.message || 'pngtosvg worker error'));
        }
      },
    };
    try {
      (0, eval)(workerSrc);
      global.self.onmessage({ data: { id: 1, image, settings } });
    } catch (e) {
      if (!done) reject(e);
    }
  });
}

function downscale(data, w, h) {
  const longest = Math.max(w, h);
  if (longest <= MAX_DIM) return { data, width: w, height: h };
  const scale = MAX_DIM / longest;
  const nw = Math.max(1, Math.round(w * scale));
  const nh = Math.max(1, Math.round(h * scale));
  const out = new Uint8ClampedArray(nw * nh * 4);
  for (let y = 0; y < nh; y++) {
    const sy = Math.min(h - 1, Math.floor(y / scale));
    for (let x = 0; x < nw; x++) {
      const sx = Math.min(w - 1, Math.floor(x / scale));
      const si = (sy * w + sx) * 4;
      const di = (y * nw + x) * 4;
      out[di] = data[si];
      out[di + 1] = data[si + 1];
      out[di + 2] = data[si + 2];
      out[di + 3] = data[si + 3];
    }
  }
  return { data: out, width: nw, height: nh };
}

async function decode() {
  const ext = path.extname(inPath).toLowerCase();
  // sharp (if present) handles everything incl. webp
  try {
    const sharp = require('sharp');
    const { data, info } = await sharp(inPath)
      .raw()
      .toBuffer({ resolveWithObject: true });
    return {
      data: new Uint8ClampedArray(data.buffer, data.byteOffset, data.byteLength),
      width: info.width,
      height: info.height,
    };
  } catch (e) {
    if (e.code !== 'MODULE_NOT_FOUND') throw e;
  }
  // pure-JS fallback: png / jpg (vendored in ./lib so the whole runtime is
  // self-contained — no npm install needed on the user machine)
  if (ext === '.png') {
    const { PNG } = require(path.join(__dirname, 'lib', 'pngjs'));
    const png = PNG.sync.read(fs.readFileSync(inPath));
    return { data: new Uint8ClampedArray(png.data.buffer, png.data.byteOffset, png.data.byteLength), width: png.width, height: png.height };
  }
  if (ext === '.jpg' || ext === '.jpeg') {
    const { decode } = require(path.join(__dirname, 'lib', 'jpeg-js'));
    const jpg = decode(fs.readFileSync(inPath), { useTArray: true, maxMemoryUsageInMB: 1024 });
    return { data: new Uint8ClampedArray(jpg.data.buffer, jpg.data.byteOffset, jpg.data.byteLength), width: jpg.width, height: jpg.height };
  }
  throw new Error(`format tidak didukung tanpa sharp: ${ext}`);
}

(async () => {
  const decoded = await decode();
  const image = downscale(decoded.data, decoded.width, decoded.height);
  let colors = COLORS;
  if (colors === 0) {
    const det = detectImageType(image);
    colors = det.colors;
    console.log(`DET ${det.type} ${det.colors}`);
  }
  const result = await runWorker(image, { colors, curve: true, cornerThreshold: 60 });
  if (!result.svg || !result.svg.includes('<svg')) {
    throw new Error('worker tidak menghasilkan SVG');
  }
  fs.writeFileSync(outPath, result.svg, 'utf8');
  console.log(`OK ${image.width}x${image.height} ${result.stats.colors}c ${result.stats.paths}p`);
})().catch((e) => {
  console.error(`ERR ${e.message}`);
  process.exit(1);
});
