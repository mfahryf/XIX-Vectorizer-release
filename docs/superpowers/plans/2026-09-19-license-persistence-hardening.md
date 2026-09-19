# XIX Vectorizer License Persistence Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make an activated XIX Vectorizer license survive a normal restart and an interrupted cache write without ever deleting the device binding or exposing the license key.

**Architecture:** Keep the existing DPAPI-protected `license-cache.lease` and `device-identity.dat` files, but replace the delete-then-rename write window with a durable replacement strategy and recovery of the previous valid cache. Add restart-focused regression coverage and make startup distinguish an unreadable status from an unactivated device.

**Tech Stack:** Rust/Tauri 2, Windows DPAPI, serde JSON, existing `LicenseStore`, existing Rust and UI test suites.

## Global Constraints

- The current running Vectorizer process must not be closed or modified as part of implementation.
- The license key, device private key, lease signature, and production secrets must never be logged or committed.
- Deleting local cache must never be used as a repair mechanism; device binding remains server-side.
- A valid cached lease must remain usable offline until its existing lease rules expire.
- Preserve the existing app-data location: `%APPDATA%\net.xixlabs.vectorizer`.

## Additional confirmed failure path: second batch reports a clock rollback

The gateway returns two different time values in a normal licensed status response:
the top-level `server_time` is the current gateway time, while the nested lease's
`server_time` is the time that lease was issued. In `store_gateway_status`, the
client first stores the current top-level value and then checks the older lease
issuance time as if it were another current server clock reading. Once a lease is
more than the five-minute recovery window old, that check can return
`ClockRollback` even though the Windows clock and gateway clock are correct. The
next `license_preflight` then surfaces the misleading “jam perangkat mundur”
error when the user starts another batch.

The fix must advance `last_server_time` only from the top-level current
`response.server_time`. The nested lease must still be signature- and
device-validated, but its historical `lease.server_time` must not overwrite the
current server clock watermark. Add a regression test with a current top-level
time and a lease issued more than five minutes earlier before changing the
implementation.

---

### Task 1: Capture the restart failure before changing storage

**Files:**
- Inspect: `src-tauri/src/licensing/storage.rs`
- Inspect: `src-tauri/src/licensing/mod.rs`
- Test: `src-tauri/src/licensing/tests.rs`

**Interfaces:**
- Consumes: `LicenseStore::save`, `LicenseStore::load`, `LicenseManager::with_client`.
- Produces: a reproducible distinction between a missing/corrupt cache and a UI status request that failed while the cache remained valid.

- [ ] **Step 1: Record read-only baseline while the app is closed by the user.**

  Record the existence and timestamps of these two files before and after one controlled exit/relaunch:

  ```powershell
  $d = "$env:APPDATA\net.xixlabs.vectorizer"
  Get-Item "$d\license-cache.lease", "$d\device-identity.dat" |
    Select-Object FullName, Length, CreationTime, LastWriteTime
  ```

  Do not delete, rename, or edit either file. If both files remain and DPAPI can still read them, the failure is in startup status handling rather than persistence.

- [ ] **Step 2: Add a restart round-trip test that creates a second manager.**

  Extend `src-tauri/src/licensing/tests.rs` with a test that saves a `LocalLicenseState` containing a signed test lease, drops the first `LicenseStore`, creates a new `LicenseStore` for the same directory, and asserts that `lease`, `lease_verified`, `last_server_time`, and the server state fields are identical after reload.

- [ ] **Step 2a: Add the batch-to-batch clock regression test.**

  Build a gateway status response whose top-level `server_time` is current and
  whose nested lease has a valid signature but an older `server_time`. Assert
  that storing it succeeds, that `last_server_time` remains the current
  top-level value, and that a second licensed preflight is still allowed.

- [ ] **Step 3: Run only the new test and confirm the baseline result.**

  Run:

  ```powershell
  cargo test --manifest-path src-tauri/Cargo.toml restart_round_trip_preserves_licensed_state
  ```

  Expected: the current serialization path passes. If it passes while the real app still appears unlicensed, capture the exact `license_status` error instead of changing the lease model.

### Task 2: Make protected cache replacement crash-safe

**Files:**
- Modify: `src-tauri/src/licensing/storage.rs`
- Test: `src-tauri/src/licensing/tests.rs`

**Interfaces:**
- Consumes: existing `write_protected` and `read_protected` APIs.
- Produces: `LicenseStore::save` that never removes the last known-good cache before the replacement is durable.

- [ ] **Step 1: Add a failing replacement test.**

  Save two different states repeatedly through the same `LicenseStore`, then assert that the final file loads as either the complete old state or the complete new state, never as an empty/truncated file. Keep the test limited to protected storage and do not test by deleting the live user cache.

- [ ] **Step 2: Write the new file completely and flush it before replacement.**

  Use a uniquely named sibling temporary file, open it with `create_new`, write the base64 payload, call `sync_all`, and close it before replacement. Keep the existing DPAPI and base64 layers unchanged.

- [ ] **Step 3: Replace the destination without a delete gap.**

  On Windows, use `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH` through a small private helper. If the replacement fails, leave the existing destination intact and return the storage error. Remove only a temporary file that was created by the failed save.

- [ ] **Step 4: Add recovery for an interrupted temporary replacement.**

  At load time, read the normal cache first. If it is absent or invalid, inspect only the known sibling temporary/backup names, validate DPAPI and JSON, and promote the first valid state back to the normal path. Never accept a cache whose lease signature or device binding fails later validation.

- [ ] **Step 5: Run storage and licensing tests.**

  Run:

  ```powershell
  cargo test --manifest-path src-tauri/Cargo.toml licensing::tests::
  cargo test --manifest-path src-tauri/Cargo.toml
  ```

### Task 3: Keep startup from presenting a valid cache as a new license

**Files:**
- Modify: `src/main.js`
- Inspect/modify only if required: `src-tauri/src/lib.rs`, `src/licensing-ui.js`
- Test: `licensing-ui.test.js`

**Interfaces:**
- Consumes: the existing `license_status` Tauri command and `deriveLicenseView`.
- Produces: a startup message that says status is temporarily unavailable when the cache is still present, instead of visually presenting the user as newly unactivated.

- [ ] **Step 1: Add a UI regression test for an unavailable status.**

  Assert that the unavailable state does not claim the user has a fresh trial or erase the existing licensed label. Keep this separate from the `unactivated` state test.

- [ ] **Step 2: Preserve the last known local licensed view during a transient startup error.**

  When `license_status` fails, keep the previous `state.license` view if one exists and show a non-destructive “status belum dapat diperiksa” message. Only show activation controls when the Rust command explicitly returns `unactivated`.

- [ ] **Step 3: Verify the UI and Rust suites.**

  Run `npm run test:ui` and `cargo test --manifest-path src-tauri/Cargo.toml`.

### Task 4: Manual restart verification and release gate

**Files:**
- Inspect: `README.md`
- Modify: `README.md` only after the behavior is verified

- [ ] **Step 1: Run the controlled manual sequence.**

  Activate a test license, close the app through its Close button, launch the same build again under the same Windows user, open LICENSE, and verify `LICENSED` or `OFFLINE LICENSE` without re-entering the key.

- [ ] **Step 2: Repeat once without network access.**

  Confirm that a still-valid lease remains usable and that the app does not reset trial counters or device identity.

- [ ] **Step 3: Run the release checklist before packaging.**

  Confirm the cache and identity files remain outside Git, run `git diff --check`, and run both the Rust and UI test suites.
