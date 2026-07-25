import fs from "node:fs";
import path from "node:path";

const rootDir = process.cwd();

function readText(relativePath) {
  return fs.readFileSync(path.join(rootDir, relativePath), "utf8");
}

function getPackageVersion() {
  const pkg = JSON.parse(readText("package.json"));
  return pkg.version;
}

function getTauriConfigVersion() {
  const tauriConfig = JSON.parse(readText("src-tauri/tauri.conf.json"));
  return tauriConfig.version;
}

function getCargoPackageVersion() {
  const cargoText = readText("src-tauri/Cargo.toml");
  const packageStart = cargoText.indexOf("[package]");
  if (packageStart === -1) {
    throw new Error("Could not find [package] section in src-tauri/Cargo.toml");
  }

  const nextSection = cargoText.indexOf("\n[", packageStart + 1);
  const packageSection = nextSection === -1
    ? cargoText.slice(packageStart)
    : cargoText.slice(packageStart, nextSection);

  const match = packageSection.match(/\nversion\s*=\s*"([^"]+)"/);
  if (!match) {
    throw new Error("Could not find package version in src-tauri/Cargo.toml");
  }

  return match[1];
}

function normalizeExpectedVersion(input) {
  if (!input) return null;
  return input.startsWith("v") ? input.slice(1) : input;
}

function assertSemverLike(version, sourceName) {
  const semverLike = /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/;
  if (!semverLike.test(version)) {
    throw new Error(`${sourceName} version is not semver-like: ${version}`);
  }
}

function main() {
  const expectedRaw = process.argv[2];
  const expected = normalizeExpectedVersion(expectedRaw);

  const versions = {
    packageJson: getPackageVersion(),
    cargoToml: getCargoPackageVersion(),
    tauriConf: getTauriConfigVersion(),
  };

  assertSemverLike(versions.packageJson, "package.json");
  assertSemverLike(versions.cargoToml, "src-tauri/Cargo.toml");
  assertSemverLike(versions.tauriConf, "src-tauri/tauri.conf.json");

  const unique = new Set(Object.values(versions));
  if (unique.size !== 1) {
    throw new Error(
      [
        "Version mismatch detected:",
        `- package.json: ${versions.packageJson}`,
        `- src-tauri/Cargo.toml: ${versions.cargoToml}`,
        `- src-tauri/tauri.conf.json: ${versions.tauriConf}`,
      ].join("\n")
    );
  }

  if (expected && versions.packageJson !== expected) {
    throw new Error(
      `Version mismatch with expected tag version: expected ${expected}, found ${versions.packageJson}`
    );
  }

  console.log(`Version sync OK: ${versions.packageJson}`);
}

try {
  main();
} catch (error) {
  console.error(error.message || error);
  process.exit(1);
}
