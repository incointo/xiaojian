# Release Documentation

This document covers release-related scripts only.

## Scripts

### npm run version:bump -- <version>

Script file:
- scripts/bump-version.mjs

What it does:
- Updates version in package.json
- Updates version in src-tauri/Cargo.toml
- Updates version in src-tauri/tauri.conf.json

Example:
```bash
npm run version:bump -- 0.1.1
```

---

### npm run version:check

Script file:
- scripts/check-versions.mjs

What it does:
- Verifies all version files are in sync:
  - package.json
  - src-tauri/Cargo.toml
  - src-tauri/tauri.conf.json

Example:
```bash
npm run version:check
```

Optional tag/version check:
```bash
npm run version:check -- v0.1.1
```

---

### npm run release:tag -- <version> [--push]

Script file:
- scripts/release-tag.mjs

What it does:
- Validates version sync first (calls check-versions)
- Creates git tag in v<version> format
- Optionally pushes the tag when --push is provided

Examples:
```bash
npm run release:tag -- 0.1.1
npm run release:tag -- 0.1.1 --push
```

## Recommended Release Flow

1. Bump all version files:
```bash
npm run version:bump -- 0.1.1
```

2. Validate version sync:
```bash
npm run version:check
```

3. Commit changes:
```bash
git add .
git commit -m "Bump version to 0.1.1"
```

4. Create and push release tag:
```bash
npm run release:tag -- 0.1.1 --push
```

5. GitHub Actions release workflow runs automatically for v* tags and creates a draft release.
