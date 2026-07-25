import { execSync } from "node:child_process";

function run(command) {
  execSync(command, { stdio: "inherit" });
}

function normalizeVersion(input) {
  return input.startsWith("v") ? input.slice(1) : input;
}

function validateVersion(version) {
  const semverLike = /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/;
  if (!semverLike.test(version)) {
    throw new Error(`Invalid version '${version}'.`);
  }
}

function main() {
  const rawVersion = process.argv[2];
  const shouldPush = process.argv.includes("--push");

  if (!rawVersion) {
    throw new Error(
      "Missing version argument. Usage: npm run release:tag -- 0.1.1 [--push]"
    );
  }

  const version = normalizeVersion(rawVersion);
  validateVersion(version);

  run(`node scripts/check-versions.mjs ${version}`);

  const tagName = `v${version}`;
  run(`git tag ${tagName}`);

  if (shouldPush) {
    run(`git push origin ${tagName}`);
  }

  console.log(`Created tag ${tagName}${shouldPush ? " and pushed it" : ""}.`);
}

try {
  main();
} catch (error) {
  console.error(error.message || error);
  process.exit(1);
}
