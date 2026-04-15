---
name: version-bump
description: Bump the synchronized version number across all dofek packages and files. Analyzes recent commits to suggest patch/minor/major, updates all version locations, stages them. Triggered by "/version-bump" or "bump version", "update version".
disable-model-invocation: true
allowed-tools: Bash(git *)
argument-hint: "[level|version]"
---

# dofek Version Bump

Bump the version number across all files that contain it. Analyze recent commits to determine the appropriate level (patch/minor/major) unless specified.

## Usage
- Auto-detect level: `/version-bump`
- Explicit level: `/version-bump minor`
- Exact version: `/version-bump 1.0.0`

## Arguments
- `$ARGUMENTS` — optional: `patch`, `minor`, `major`, or an exact semver like `1.2.3`

## Steps

### 1. Determine the new version

Get the current version:
```
!`grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/'`
```

Review recent commits since last version change:
```
!`git log --oneline -10`
```

If `$ARGUMENTS` is empty, analyze the commits and suggest:
- **patch** — bug fixes, small tweaks, perf optimizations
- **minor** — new features, UI improvements, new files/modules
- **major** — breaking changes, architecture rewrites

If `$ARGUMENTS` is `patch`, `minor`, or `major`, compute the new version by incrementing.
If `$ARGUMENTS` is an exact semver (e.g., `1.0.0`), use it directly.

### 2. Update ALL version locations

These are the files and patterns to update. **All must be updated together.**

| File | Pattern | Example |
|------|---------|---------|
| `Cargo.toml` (root) | `version = "X.Y.Z"` | `version = "0.5.0"` |
| `gui/Cargo.toml` | `version = "X.Y.Z"` | `version = "0.5.0"` |
| `gui/tauri.conf.json` | `"version": "X.Y.Z"` | `"version": "0.5.0"` |
| `src/ui/ticker.rs` | `" vX.Y"` | `" v0.5"` |
| `src/ui/about.rs` | `"vX.Y"` | `"v0.5"` |
| `gui/frontend/index.html` | `v0.Y` (logo + about modal) | `v0.5` |
| `gui/docs/dofek-concept-v2.html` | `v0.Y` | `v0.5` |
| `website/index.html` | `v0.Y` (nav, footer, hero) + `"softwareVersion": "X.Y"` | `v0.5` |
| `website/plugins/index.html` | `Plugin System vX.Y` | `Plugin System v0.5` |
| `README.md` | `dofek vX.Y` (screenshot) + `dofek_X.Y.Z_x64_en-US.msi` + roadmap `(current)` marker |
| `CLAUDE.md` | `dofek_X.Y.Z_x64_en-US.msi` + `Current Status (vX.Y)` |

**Important:** Use the short form `vX.Y` (not `vX.Y.Z`) in display strings (ticker, about, website, GUI). Use full `X.Y.Z` in Cargo.toml, tauri.conf.json, MSI filenames, and JSON-LD.

### 3. Update the roadmap

In `README.md`, move the `(current)` marker to the new version's line in the Roadmap section. If the new version doesn't have a roadmap entry yet, add one with a brief summary based on the commits.

### 4. Stage and confirm

Stage all modified files with `git add` (list them explicitly, don't use `-A`).

Show the user:
- Previous version → New version
- List of files updated
- The diff summary

Ask for confirmation before committing. Do NOT commit automatically.
