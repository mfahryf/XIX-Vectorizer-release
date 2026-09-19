# XIX Vectorizer Desktop Updater and GitHub Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish a signed Windows desktop release from a new public repository and let installed Vectorizer builds check, download, and install future releases through Tauri’s updater.

**Architecture:** Create the public repository `mfahryf/XIX-Vectorizer-release` from the existing private desktop source, keep the existing private repository untouched until the public copy is verified, and use GitHub Releases as both the release store and static updater metadata host. The updater endpoint will always use the stable `releases/latest/download/latest.json` URL, so no installed binary needs a version-specific endpoint.

**Tech Stack:** Tauri 2, `tauri-plugin-updater`, `@tauri-apps/plugin-updater`, Tauri signing key, GitHub Actions, `tauri-apps/tauri-action@v1`, Windows NSIS.

## Global Constraints

- The public repository must contain no license cache, device identity, usage ledger, production secret, or Mayar credential.
- The Tauri updater signature is mandatory; the private signing key must remain only in GitHub Actions secrets.
- The public key may be committed to `src-tauri/tauri.conf.json`.
- Initial target is Windows x64 NSIS because the current bundle targets only `nsis` and the app is currently used on Windows.
- Updates must never install while a batch is running; the user must confirm installation.
- Use the stable repository name `mfahryf/XIX-Vectorizer-release`.

---

### Task 1: Prepare the public repository safely

**Files:**
- Inspect: `.gitignore`, `README.md`, `src-tauri/tauri.conf.json`, `package.json`, `src-tauri/Cargo.toml`
- Create externally: public GitHub repository `mfahryf/XIX-Vectorizer-release`

- [ ] **Step 1: Verify the current private repository has no tracked local state.**

  Run:

  ```powershell
  git ls-files | rg -i '(^|/)(\.env|.*\.key|.*\.pem|.*\.lease|device-identity|license-cache|usage-ledger|secret|credential)'
  ```

  Expected: no runtime cache or production secret paths.

- [ ] **Step 2: Create the new repository as public and push a copy.**

  Use `gh repo create mfahryf/XIX-Vectorizer-release --public --source . --remote public --push` only after the owner confirms the repository name. Do not alter the existing `origin` remote until the public copy has been verified.

- [ ] **Step 3: Verify public visibility and source hygiene.**

  Run `gh repo view mfahryf/XIX-Vectorizer-release --json visibility,isPrivate,url` and inspect the public repository file list. Confirm no cache or secret file is present.

### Task 2: Add signed updater support

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `package.json`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/capabilities/default.json`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/main.js`
- Modify: `src/index.html`
- Modify: `src/style.css`
- Test: `licensing-ui.test.js`, new updater UI test if needed

**Interfaces:**
- Consumes: static updater metadata at `https://github.com/mfahryf/XIX-Vectorizer-release/releases/latest/download/latest.json`.
- Produces: a visible, user-confirmed update flow that reports checking, available version, download progress, success, and failure without blocking normal processing.

- [ ] **Step 1: Generate and protect the updater signing key.**

  Generate one key pair with the Tauri signer. Store the private key and password in GitHub repository secrets named `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; commit only the generated public key content to `tauri.conf.json`.

- [ ] **Step 2: Add the updater dependencies and permissions.**

  Add `tauri-plugin-updater` for desktop targets and `@tauri-apps/plugin-updater` to `package.json`. Register the updater plugin in `lib.rs`, and add `updater:default` to `capabilities/default.json`. Add the process plugin only if the final UI explicitly calls `relaunch`.

- [ ] **Step 3: Configure updater artifacts and endpoint.**

  Add this configuration shape to `src-tauri/tauri.conf.json`:

  ```json
  "bundle": {
    "createUpdaterArtifacts": true
  },
  "plugins": {
    "updater": {
      "pubkey": "the exact public-key text printed by the Tauri signer",
      "endpoints": [
        "https://github.com/mfahryf/XIX-Vectorizer-release/releases/latest/download/latest.json"
      ],
      "windows": {
        "installMode": "passive"
      }
    }
  }
  ```

  The key content is supplied by the signer output, not invented or replaced with a path.

- [ ] **Step 4: Add a manual update button and a safe startup check.**

  Add an UPDATE control to the existing title/toolbar layout. Check once after initialization and expose the same check through the button. If a batch is running, show the update as pending and refuse installation until the batch is idle. On Windows, use `downloadAndInstall`; show progress and let the installer exit the app as required by the platform.

- [ ] **Step 5: Test no-update, update-available, cancel, failure, and running-batch cases.**

  Add deterministic UI tests for the visible states and keep network/update calls behind a small module boundary so tests do not contact GitHub.

### Task 3: Automate Windows build and GitHub Release

**Files:**
- Create: `.github/workflows/ci.yml`
- Create: `.github/workflows/release.yml`
- Modify: `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`

- [ ] **Step 1: Add pull-request CI.**

  Run `npm ci`, `npm run test:ui`, and `cargo test --manifest-path src-tauri/Cargo.toml` on `windows-latest`. Do not expose signing secrets in this workflow.

- [ ] **Step 2: Add signed release workflow.**

  Trigger on `workflow_dispatch` and version tags. Grant only `contents: write`. Run `npm ci`, Rust setup/cache, then `tauri-apps/tauri-action@v1` with the signing secrets and `GITHUB_TOKEN`. After publishing, upload a copy of the installer as `Vectorizer-latest-x64-setup.exe` so the public web page can link directly to the newest installer without embedding a version number.

- [ ] **Step 3: Produce the first updater-compatible release.**

  Bump all application version declarations from `0.1.0` to `0.1.1`, run the full tests, push the `release` branch, inspect the draft release assets, verify `latest.json`, and publish the draft only after the signature and Windows installer asset are present.

- [ ] **Step 4: Verify updater metadata independently.**

  Fetch `https://github.com/mfahryf/XIX-Vectorizer-release/releases/latest/download/latest.json` and verify that the Windows x64 entry contains a versioned installer URL and a non-empty signature. Also verify that `https://github.com/mfahryf/XIX-Vectorizer-release/releases/latest/download/Vectorizer-latest-x64-setup.exe` returns the installer directly. Do not install it on the currently running app until the license persistence test is complete.

### Task 4: Update documentation and repository registry

**Files:**
- Modify: `README.md`
- Modify: `XIXLabs.net/registry/applications.yaml`

- [ ] **Step 1: Document release and signing ownership.**

  Record the public repository, release branch/tag convention, required secrets by name only, and the rule that losing the private signing key prevents future updates to already installed builds.

- [ ] **Step 2: Update the registry.**

  Change `desktop_repository` to `mfahryf/XIX-Vectorizer-release`, add `desktop_repository_visibility: public`, and keep the existing private payment/license gateway references private.
