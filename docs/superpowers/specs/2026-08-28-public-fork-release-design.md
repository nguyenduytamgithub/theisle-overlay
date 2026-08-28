# Public Navigation Fork Release Design

## Goal

Publish the tested navigation build as a public GitHub fork under
`nguyenduytamgithub/theisle-overlay`, with enough Vietnamese and English
documentation for a new user to understand its provenance, advantages,
installation, controls, limitations, and recovery path without reading source.

## Provenance and positioning

- The fork is based on upstream public tag `v1.5.2` at commit `f628a18`.
- The original project is `toantranct/theisle-overlay`; GitHub's fork network and
  the first README screen must both preserve that attribution.
- Upstream `2.0.0+` is release-only and includes closed-source Pro features. This
  fork must compare itself only with the open-source `v1.5.2` baseline and must
  explicitly say it does not include 2.x voice, friend-position, skin-editor, or
  paid realtime features.
- Upstream currently has no `LICENSE` file. The fork must not invent or claim a
  license. The README will state that upstream licensing terms are not declared.

## Public documentation

`README.md` is the primary Vietnamese landing page and `README.en.md` mirrors the
important claims. Both will include:

1. A prominent community-fork banner with links to upstream and the fork maintainer.
2. A comparison table against upstream open-source `v1.5.2`.
3. A concise explanation of the navigation pipeline: five-second server polling,
   four-second bounded visual prediction, 350 ms correction, outlier quarantine,
   server-yaw preference, and stale-state labelling.
4. Exact install steps from the fork's GitHub Release and a SmartScreen warning.
5. Exact usage for creating a waypoint and choosing **Dẫn đường tới điểm này**.
6. Hotkeys, especially `Ctrl+Alt+H`, `Ctrl+Alt+M`, `Ctrl+Alt+F`, and
   `Ctrl+Alt+R`.
7. Honest limits: server confirmation is not true local realtime, manual
   `Tab -> Asset Location` remains necessary without IslePilot live-map support,
   Borderless/Windowed mode is required, and this fork uses manual updates.
8. Anti-cheat boundaries: HTTPS/clipboard only; no memory read, DLL injection,
   hooks, packet capture, or synthetic input.

## GitHub publication

- Create the public GitHub fork `nguyenduytamgithub/theisle-overlay`.
- Push the tested commit history to branch `navigation-hud` and make that the
  fork's default branch. Keep fork `main` tracking upstream instead of force
  overwriting it.
- Set a clear repository description and navigation-related topics.
- Point the updater endpoint at the public fork and disable signed updater
  artifact generation. This prevents the fork from offering upstream 2.x while
  keeping manual NSIS Releases reproducible without the upstream private key.
- Tag the tested commit as `v1.6.0-navigation-hud`.
- Create a public GitHub Release from a checked-in release-notes file and attach
  the already verified unsigned NSIS installer.
- Include the installer's SHA-256 in release notes. Do not publish credentials,
  cookies, tokens, private URLs, updater signing keys, or local settings.

## Verification

- README links must target the public fork release and the exact upstream repo.
- Vietnamese and English pages must agree on version, polling, prediction,
  shortcuts, and limitations.
- `git diff --check`, the existing navigation tests, Svelte checks, Rust tests,
  Clippy, and forbidden-API checks must pass before publication.
- After pushing, use GitHub API readback to verify public visibility, fork parent,
  default branch, tag, release, asset size, and asset download URL.
