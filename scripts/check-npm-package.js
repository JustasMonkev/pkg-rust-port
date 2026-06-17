"use strict";

const { spawnSync } = require("node:child_process");
const fs = require("node:fs");

const rootPackage = JSON.parse(fs.readFileSync("package.json", "utf8"));

const requiredRootFields = ["name", "version", "bin", "files", "engines", "repository", "license", "homepage", "keywords"];
for (const field of requiredRootFields) {
  if (!rootPackage[field]) {
    throw new Error(`package.json is missing ${field}`);
  }
}

if (rootPackage.name !== "@justasmonkev/pkg-rust") {
  throw new Error(`unexpected npm package name: ${rootPackage.name}`);
}

if (rootPackage.bin.pkg !== "./bin/pkg.js") {
  throw new Error("package.json bin.pkg must point at ./bin/pkg.js");
}

if (!rootPackage.files.includes("bin/pkg.js") || rootPackage.files.some((entry) => entry.startsWith("target"))) {
  throw new Error("package.json files allowlist must include bin/pkg.js and exclude target/");
}

const nativePackages = new Set(Object.keys(rootPackage.optionalDependencies || {}));
const expectedPackages = new Set([
  "@justasmonkev/pkg-rust-darwin-arm64",
  "@justasmonkev/pkg-rust-darwin-x64",
  "@justasmonkev/pkg-rust-linux-arm64-gnu",
  "@justasmonkev/pkg-rust-linux-arm64-musl",
  "@justasmonkev/pkg-rust-linux-x64-gnu",
  "@justasmonkev/pkg-rust-linux-x64-musl",
  "@justasmonkev/pkg-rust-win32-x64-msvc"
]);

for (const name of expectedPackages) {
  if (!nativePackages.has(name)) {
    throw new Error(`missing optionalDependency ${name}`);
  }
}

for (const name of nativePackages) {
  if (!expectedPackages.has(name)) {
    throw new Error(`unexpected optionalDependency ${name}`);
  }
}

const packageDirs = fs.readdirSync("npm", { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name);

if (packageDirs.length !== expectedPackages.size) {
  throw new Error(`expected ${expectedPackages.size} native package directories, found ${packageDirs.length}`);
}

for (const packageDir of packageDirs) {
  const packagePath = `npm/${packageDir}/package.json`;
  const nativePackage = JSON.parse(fs.readFileSync(packagePath, "utf8"));
  if (!expectedPackages.has(nativePackage.name)) {
    throw new Error(`${packagePath} has unexpected name ${nativePackage.name}`);
  }
  for (const field of ["name", "version", "files", "os", "cpu", "license", "repository", "homepage", "keywords"]) {
    if (!nativePackage[field]) {
      throw new Error(`${packagePath} is missing ${field}`);
    }
  }
}

const launcher = spawnSync(process.execPath, [
  "bin/pkg.js",
  "-e",
  "console.log('launcher-ok:' + process.argv.slice(1).join(','));",
  "one",
  "two"
], {
  cwd: process.cwd(),
  env: { ...process.env, PKG_RUST_BINARY_PATH: process.execPath },
  encoding: "utf8"
});

if (launcher.status !== 0) {
  throw new Error(`launcher failed: ${launcher.stderr || launcher.stdout}`);
}

if (launcher.stdout.trim() !== "launcher-ok:one,two") {
  throw new Error(`launcher did not forward arguments correctly: ${launcher.stdout}`);
}
