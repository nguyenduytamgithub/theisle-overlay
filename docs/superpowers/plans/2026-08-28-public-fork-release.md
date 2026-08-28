# Public Navigation Fork Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish the tested navigation build as a transparent, documented, installable public GitHub fork.

**Architecture:** Keep navigation behavior unchanged, route updater metadata to the fork, and disable signed updater artifact generation so releases are explicitly manual. Add bilingual public documentation plus durable release notes, push the existing commit history to a dedicated default branch in a GitHub fork, and publish the rebuilt NSIS artifact as a manual-install release.

**Tech Stack:** Markdown, Git, GitHub CLI/API, Tauri NSIS artifact, Node/Svelte, Rust/Cargo.

## Global Constraints

- Attribute `https://github.com/toantranct/theisle-overlay` and identify `v1.5.2` / `f628a18` as the open-source baseline.
- Do not claim parity with upstream 2.x or include its closed-source Pro features.
- Do not add a license when upstream has no declared `LICENSE` file.
- Do not publish secrets, authentication data, private URLs, or local settings.
- Do not force-push or replace the fork's upstream-tracking `main` branch.
- Release updates are manual because this fork has no updater-signing private key.

---

### Task 1: Bilingual public documentation

**Files:**
- Modify: `README.md`
- Modify: `README.en.md`
- Create: `docs/releases/v1.6.0-navigation-hud.md`

**Interfaces:**
- Consumes: implemented v1.6.0 behavior and the verified installer checksum.
- Produces: public landing pages and the exact GitHub Release body.

- [ ] **Step 1: Add provenance and comparison sections**

State the upstream tag/commit, fork maintainer, 2.x exclusion, and feature-by-feature navigation differences using literal verified values.

- [ ] **Step 2: Add install and usage walkthroughs**

Document fork Release download, SmartScreen, Borderless/Windowed mode, IslePilot/manual coordinate paths, waypoint selection, HUD states, and recovery hotkeys.

- [ ] **Step 3: Add durable release notes**

Record the feature list, limits, upstream attribution, installer size, and SHA-256 in `docs/releases/v1.6.0-navigation-hud.md`.

- [ ] **Step 4: Verify documentation consistency**

Run:

```powershell
rg -n "v1\.5\.2|f628a18|v1\.6\.0-navigation-hud|Ctrl\+Alt\+H|5 giây|5 seconds|4 giây|4 seconds|toantranct" README.md README.en.md docs/releases/v1.6.0-navigation-hud.md
git diff --check
```

- [ ] **Step 5: Commit**

```powershell
git add README.md README.en.md docs/releases/v1.6.0-navigation-hud.md
git commit -m "docs: prepare public navigation fork release"
```

### Task 2: Regression and safety verification

**Files:**
- Create: `src-tauri/tests/release_config.rs`
- Modify: `src-tauri/tauri.conf.json`
- Verify: tracked source tree and rebuilt NSIS installer.

**Interfaces:**
- Consumes: the final documented commit.
- Produces: a publish/no-publish gate with fresh command evidence.

- [ ] **Step 1: Add failing fork-release configuration tests**

The tests load the real Tauri configuration and require the sole updater
endpoint to target `nguyenduytamgithub/theisle-overlay` and
`createUpdaterArtifacts` to be `false`. Run the focused test before changing
the configuration and require both assertions to fail for those exact reasons.

- [ ] **Step 2: Switch to the manual fork update channel**

Change `src-tauri/tauri.conf.json` to the fork's `latest.json` URL and disable
updater artifact creation, then rerun `cargo test --test release_config`.

- [ ] **Step 3: Run frontend tests and checks**

```powershell
node --test src/lib/navigation/guidance.test.mjs src/lib/navigation/prediction.test.mjs
npm run check
npm run build
```

- [ ] **Step 4: Run Rust tests and static analysis**

```powershell
cargo test --workspace --manifest-path src-tauri/Cargo.toml
cargo clippy --workspace --all-targets --manifest-path src-tauri/Cargo.toml -- -D warnings
```

- [ ] **Step 5: Run the forbidden-API and secret gates**

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check-forbidden-apis.ps1
git grep -n -I -E "(gho_|ghp_|BEGIN (RSA|OPENSSH|EC) PRIVATE KEY|islepilot_player=)" -- .
```

Expected: forbidden API script passes; secret grep returns no matches.

- [ ] **Step 6: Rebuild and confirm installer identity**

```powershell
npm run tauri build
git status --short
Get-FileHash 'src-tauri/target/release/bundle/nsis/TheIsle Overlay_1.6.0_x64-setup.exe' -Algorithm SHA256
```

### Task 3: Public GitHub fork and release

**Files:**
- External state: `https://github.com/nguyenduytamgithub/theisle-overlay`

**Interfaces:**
- Consumes: verified Git commit, release notes, and NSIS installer.
- Produces: public fork default branch, tag, release page, and downloadable asset.

- [ ] **Step 1: Create the fork without cloning**

```powershell
gh repo fork toantranct/theisle-overlay --clone=false --remote=false
```

- [ ] **Step 2: Add a dedicated remote and push without force**

```powershell
git remote add public-fork https://github.com/nguyenduytamgithub/theisle-overlay.git
git push public-fork HEAD:refs/heads/navigation-hud
```

- [ ] **Step 3: Configure the public repository**

Use GitHub API/CLI to set `navigation-hud` as default, apply the description,
homepage/upstream link, and topics `the-isle`, `evrima`, `overlay`, `minimap`,
`navigation`, `tauri`, and `windows`.

- [ ] **Step 4: Tag and publish the release**

```powershell
git tag -a v1.6.0-navigation-hud -m "TheIsle Overlay Navigation HUD v1.6.0"
git push public-fork v1.6.0-navigation-hud
gh release create v1.6.0-navigation-hud --repo nguyenduytamgithub/theisle-overlay --target navigation-hud --title "TheIsle Overlay Navigation HUD v1.6.0" --notes-file docs/releases/v1.6.0-navigation-hud.md 'src-tauri/target/release/bundle/nsis/TheIsle Overlay_1.6.0_x64-setup.exe'
```

- [ ] **Step 5: Read back public state**

Verify repository visibility, parent, default branch, tag commit, release state,
asset byte size, and browser/download URLs with `gh repo view`, `gh api`, and
`gh release view`. Open the final repository page for the user.
