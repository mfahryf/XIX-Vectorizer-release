(function (root, factory) {
  if (typeof module === "object" && module.exports) {
    module.exports = factory();
  } else {
    root.XixEngineUI = factory();
  }
})(typeof globalThis === "object" ? globalThis : this, function () {
  const ENGINE_ORDER = {
    "vectorize-v1": 0,
    "vectorize-v2": 1,
    pngtosvg: 2,
  };
  const ENGINE_MODES = {
    "vectorize-v1": "Online",
    "vectorize-v2": "Online",
    pngtosvg: "Offline",
  };
  const SUPERSCRIPT = {
    O: "ᴼ",
    n: "ⁿ",
    l: "ˡ",
    i: "ᶦ",
    e: "ᵉ",
    f: "ᶠ",
  };

  function sortEngines(engines) {
    return [...engines].sort((left, right) => {
      const leftOrder = ENGINE_ORDER[left.id] ?? Number.MAX_SAFE_INTEGER;
      const rightOrder = ENGINE_ORDER[right.id] ?? Number.MAX_SAFE_INTEGER;
      return leftOrder - rightOrder || String(left.name).localeCompare(String(right.name));
    });
  }

  function formatEngineLabel(engine) {
    const mode = ENGINE_MODES[engine.id] || "Online";
    const superscriptMode = [...mode].map((character) => SUPERSCRIPT[character] || character).join("");
    return engine.name + " ⁽" + superscriptMode + "⁾";
  }

  return { formatEngineLabel, sortEngines };
});
