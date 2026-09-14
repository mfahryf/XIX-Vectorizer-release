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

test("fresh installs explain that processing activates the trial", () => {
  const view = deriveLicenseView({
    license_state: "unactivated",
    device_state: "unregistered",
    trial_remaining_by_engine: { "vectorize-v1": 5, "vectorize-v2": 5, pngtosvg: 5 },
  });
  assert.equal(view.canProcess, false);
  assert.equal(view.canStart, true);
  assert.equal(view.badge, "NOT ACTIVATED");
  assert.match(view.message, /file pertama/i);
  assert.equal(view.deviceState, "unregistered");
});

test("device identity loss exposes recovery code and contact metadata", () => {
  const view = deriveLicenseView({
    license_state: "device-identity-lost",
    device_state: "identity-lost",
    recovery_request_code: "AB12CD34EF56",
    recovery_contact: "hubungi admin lisensi XIXLabs",
  });
  assert.equal(view.canProcess, false);
  assert.equal(view.badge, "RECOVERY");
  assert.equal(view.recoveryRequestCode, "AB12CD34EF56");
  assert.equal(view.recoveryContact, "hubungi admin lisensi XIXLabs");
  assert.equal(view.deviceState, "identity-lost");
});

test("clock rollback is shown as a local validation problem", () => {
  const view = deriveLicenseView({ license_state: "clock-rollback" });
  assert.equal(view.canProcess, false);
  assert.equal(view.badge, "CLOCK CHECK");
  assert.match(view.message, /waktu perangkat/i);
});
