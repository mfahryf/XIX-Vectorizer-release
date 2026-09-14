const test = require("node:test");
const assert = require("node:assert/strict");
const { deriveLicenseView } = require("./src/licensing-ui.js");

test("expired lease locks processing but leaves recovery actions", () => {
  const view = deriveLicenseView({
    license_state: "expired-offline",
    trial_remaining_by_engine: { "vectorize-v1": 0, "vectorize-v2": 2, pngtosvg: 5 },
  });
  assert.equal(view.canProcess, false);
  assert.equal(view.showActivation, true);
  assert.equal(view.message, "Hubungkan internet untuk memvalidasi lisensi.");
});

test("trial view exposes a separate counter for every engine", () => {
  const view = deriveLicenseView({
    license_state: "trial",
    trial_remaining_by_engine: { "vectorize-v1": 5, "vectorize-v2": 2, pngtosvg: 0 },
  });
  assert.equal(view.canProcess, true);
  assert.deepEqual(view.engineCounters, [
    { id: "vectorize-v1", remaining: 5 },
    { id: "vectorize-v2", remaining: 2 },
    { id: "pngtosvg", remaining: 0 },
  ]);
  assert.equal(view.badge, "TRIAL");
});

test("active and offline leases remain usable without activation prompt", () => {
  for (const state of ["licensed", "licensed-offline"]) {
    const view = deriveLicenseView({ license_state: state });
    assert.equal(view.canProcess, true);
    assert.equal(view.showActivation, false);
  }
});
