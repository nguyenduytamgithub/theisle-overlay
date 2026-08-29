# Navigation v1.7.4 Integration Design

**Status:** Approved by the user on 2026-08-29

## Goal

Integrate the complete deterministic newbie-navigation release at
`public/navigation-hud` commit `21ae583` into the installed Night Vision and
IslePilot-resilience line at `c206501`, without removing either line's behavior
or modifying the running game until verification passes.

## Chosen approach

Use one non-fast-forward Git merge so the other agent's tested commit history
remains auditable. Resolve conflicts manually with these priorities:

1. Navigation source and tests come from `public/navigation-hud` unless the
   current branch contains a later independent Night Vision or IslePilot change.
2. Night Vision magnifier boost-b, the v1.7.3 IslePilot timeout/retry fix, and
   all associated safety tests remain intact.
3. Release metadata becomes v1.7.4; no file may regress to v1.7.0 or v1.7.2.
4. Existing encrypted token, settings, waypoints, trails, and game process are
   preservation targets, not replaceable build inputs.

Cherry-picking only `dfa8b46` is rejected because it depends on the preceding
estimator, metadata, HUD, minimap, and recovery commits. Reimplementing the
feature is rejected because the source branch already has a complete red-green
test history and a clean baseline.

## Conflict and data boundaries

The highest-risk shared areas are HUD lifecycle/stacking, navigation event
payloads, settings migration, release configuration, and user documentation.
Night Vision remains a separate overlay window and must not consume navigation
state. Navigation remains passive and must not read game memory, inject code,
hook input, synthesize input, capture packets, or persist predicted positions.

The schema-v1 default arrival radius migration from 15 m to 25 m is allowed.
Custom arrival radii and all unrelated settings must survive byte-for-byte or
semantic comparison as appropriate. Installation may stop only the exact
installed overlay process; The Isle must remain responsive on the same PID.

## Verification

Before merge, verify both branch baselines. After conflict resolution, require:

- 34/34 navigation tests;
- Svelte check with zero errors and warnings plus a production Vite build;
- full Rust workspace tests and Clippy with warnings denied;
- Night Vision safety and release-configuration tests;
- forbidden-API and credential-shaped-secret scans;
- CodeGraph resynchronization and a focused integration-path query;
- v1.7.4 NSIS build, artifact hash, exact user-data preservation checks, and
  responsive installed overlay plus unchanged game PID.

Automated and process checks prove the integration and installation. Final live
navigation acceptance still requires the user to select one waypoint and move
through at least three confirmed server updates; it must be reported separately
and never inferred from a green build.
