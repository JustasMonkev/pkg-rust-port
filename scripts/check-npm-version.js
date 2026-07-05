"use strict";

const fs = require("node:fs");

const rootPackage = JSON.parse(fs.readFileSync("package.json", "utf8"));
const cargoToml = fs.readFileSync("Cargo.toml", "utf8");
const cargoVersionMatch = cargoToml.match(/^version\s*=\s*"([^"]+)"/m);

if (!cargoVersionMatch) {
  throw new Error("Cargo.toml package version was not found");
}

const cargoVersion = cargoVersionMatch[1];
if (rootPackage.version !== cargoVersion) {
  throw new Error(`package.json version ${rootPackage.version} does not match Cargo.toml version ${cargoVersion}`);
}

const optionalDependencies = rootPackage.optionalDependencies || {};
for (const [name, version] of Object.entries(optionalDependencies)) {
  if (version !== cargoVersion) {
    throw new Error(`${name} optionalDependency version ${version} does not match ${cargoVersion}`);
  }
}

for (const packageDir of fs.readdirSync("npm", { withFileTypes: true })) {
  if (!packageDir.isDirectory()) {
    continue;
  }
  const packagePath = `npm/${packageDir.name}/package.json`;
  const nativePackage = JSON.parse(fs.readFileSync(packagePath, "utf8"));
  if (nativePackage.version !== cargoVersion) {
    throw new Error(`${packagePath} version ${nativePackage.version} does not match ${cargoVersion}`);
  }
}
