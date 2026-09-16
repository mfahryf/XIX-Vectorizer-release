// Image-type detection for the V3 engine: analyze the RGBA image and
// classify it as sketch / logo / photo, which determines the target color
// count.
//
//   sketch     → 3 colors   (line art, two tones)
//   logo       → 8 colors   (flat colors, crisp edges)
//   photo      → 24 colors  (many colors, soft gradients)
//
// Heuristics:
//   c  = average chroma of non-dominant pixels
//   g  = flatness — fraction of neighbouring pixels that are near-identical
//   y  = bins90 — how many quantized colors cover the first 90% of pixels
// sketch is chosen when (c < 12 && g > 0.5) and logo when (y <= 48 &&
// g > 0.55), otherwise photo. The worker still optimizes the final palette
// below the target, so these are upper bounds.

/**
 * @param {{data: Uint8ClampedArray, width: number, height: number}} image
 * @returns {{type: "sketch"|"logo"|"photo", colors: number}}
 */
function detectImageType(image) {
  const { data: t, width: n, height: r } = image;
  const l = n * r;
  const i = Math.max(1, Math.floor(l / 24000)); // sample ~24k pixels
  const o = new Map();
  let a = 0; // similar-neighbour count
  let u = 0; // neighbour samples
  let d = 0; // total samples

  // straight-alpha composite onto white (same as worker's `mn`)
  const h = (C) => {
    const R = (t[C * 4 + 3] / 255);
    return [
      t[C * 4] * R + 255 * (1 - R),
      t[C * 4 + 1] * R + 255 * (1 - R),
      t[C * 4 + 2] * R + 255 * (1 - R),
    ];
  };

  for (let C = 0; C < l; C += i) {
    const [R, z, T] = h(C);
    const V = Math.max(R, z, T) - Math.min(R, z, T); // chroma
    const W = ((R & 240) << 4) | (z & 240) | (T >> 4); // 12-bit quantized color
    const bin = o.get(W);
    if (bin) {
      bin[0]++;
      bin[1] += V;
    } else {
      o.set(W, [1, V]);
    }
    d++;
    if (C % n + 1 < n) {
      const [E, D, G] = h(C + 1);
      if (Math.abs(R - E) + Math.abs(z - D) + Math.abs(T - G) < 12) a++;
      u++;
    }
  }

  const g = u ? a / u : 1; // flatness
  const m = [...o.values()].sort((p, q) => q[0] - p[0]); // by count desc
  let S = 0; // total samples
  let w = 0; // total chroma
  for (const [cnt, ch] of m) {
    S += cnt;
    w += ch;
  }
  const v = m[0][0]; // dominant-bin count
  const j = m[0][1]; // dominant-bin chroma
  const p = S - v; // non-dominant samples
  const c = p > S * 0.01 ? (w - j) / p : w / S; // avg chroma of the rest

  let f = 0;
  let y = 0;
  const k = d * 0.9;
  for (const [cnt] of m) {
    f += cnt;
    y++;
    if (f >= k) break;
  }

  let type;
  if (c < 12 && g > 0.5) type = 'sketch';
  else if (y <= 48 && g > 0.55) type = 'logo';
  else type = 'photo';
  const colors = type === 'sketch' ? 3 : type === 'logo' ? 8 : 24;
  return { type, colors };
}

module.exports = { detectImageType };
