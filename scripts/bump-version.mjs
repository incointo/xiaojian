import fs from "node:fs";
import path from "node:path";

const rootDir = process.cwd();

function filePath(relativePath) {
  return path.join(rootDir, relativePath);
}

function readText(relativePath) {
  return fs.readFileSync(filePath(relativePath), "utf8");
}

function writeText(relativePath, content) {
  fs.writeFileSync(filePath(relativePath), content, "utf8");
}

function validateVersion(version) {
  const semverLike = /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/;
  if (!semverLike.test(version)) {
    throw new Error(
      `Invalid version '${version}'. Use semver format, e.g. 0.1.1 or 1.2.3-beta.1`
    );
  }
}

function updatePackageJson(version) {
  const pkgPath = "package.json";
  const pkg = JSON.parse(readText(pkgPath));
  pkg.version = version;
  writeText(pkgPath, `${JSON.stringify(pkg, null, 2)}\n`);
}

function updateCargoToml(version) {
  const cargoPath = "src-tauri/Cargo.toml";
  const cargoText = readText(cargoPath);
  const updated = cargoText.replace(
    /(\[package\][\s\S]*?\nversion\s*=\s*")([^"]+)(")/,
    `$1${version}$3`
  );

  if (updated === cargoText) {
    throw new Error("Could not update version in src-tauri/Cargo.toml");
  }

  writeText(cargoPath, updated);
}

function updateTauriConf(version) {
  const tauriPath = "src-tauri/tauri.conf.json";
  const tauriConf = JSON.parse(readText(tauriPath));
  tauriConf.version = version;
  writeText(tauriPath, `${JSON.stringify(tauriConf, null, 2)}\n`);
}

function main() {
  const nextVersion = process.argv[2];
  if (!nextVersion) {
    throw new Error("Missing version argument. Usage: npm run version:bump -- 0.1.1");
  }

  validateVersion(nextVersion);

  updatePackageJson(nextVersion);
  updateCargoToml(nextVersion);
  updateTauriConf(nextVersion);

  console.log(`Updated versions to ${nextVersion}:`);
  console.log("- package.json");
  console.log("- src-tauri/Cargo.toml");
  console.log("- src-tauri/tauri.conf.json");
}

try {
  main();
} catch (error) {
  console.error(error.message || error);
  process.exit(1);
}
