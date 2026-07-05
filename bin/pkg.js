#!/usr/bin/env node
"use strict";

const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const PACKAGE_BY_PLATFORM = {
  "darwin arm64": "@justasmonkev/pkg-rust-darwin-arm64",
  "darwin x64": "@justasmonkev/pkg-rust-darwin-x64",
  "linux arm64 glibc": "@justasmonkev/pkg-rust-linux-arm64-gnu",
  "linux arm64 musl": "@justasmonkev/pkg-rust-linux-arm64-musl",
  "linux x64 glibc": "@justasmonkev/pkg-rust-linux-x64-gnu",
  "linux x64 musl": "@justasmonkev/pkg-rust-linux-x64-musl",
  "win32 x64": "@justasmonkev/pkg-rust-win32-x64-msvc"
};

function linuxLibc() {
  if (process.platform !== "linux") {
    return "";
  }

  const report = process.report && typeof process.report.getReport === "function"
    ? process.report.getReport()
    : undefined;
  return report && report.header && report.header.glibcVersionRuntime ? "glibc" : "musl";
}

function packageNameForCurrentPlatform() {
  const parts = [process.platform, process.arch];
  const libc = linuxLibc();
  if (libc) {
    parts.push(libc);
  }
  return PACKAGE_BY_PLATFORM[parts.join(" ")];
}

function binaryName() {
  return process.platform === "win32" ? "pkg.exe" : "pkg";
}

function resolveBinaryPath() {
  if (process.env.PKG_RUST_BINARY_PATH) {
    return process.env.PKG_RUST_BINARY_PATH;
  }

  const packageName = packageNameForCurrentPlatform();
  if (!packageName) {
    throw new Error(`Unsupported platform: ${process.platform} ${process.arch}`);
  }

  let packageJsonPath;
  try {
    packageJsonPath = require.resolve(`${packageName}/package.json`);
  } catch (error) {
    const installHint = "Make sure optional dependencies were installed, or reinstall without --omit=optional.";
    throw new Error(`Missing native package ${packageName}. ${installHint}`);
  }

  return path.join(path.dirname(packageJsonPath), "bin", binaryName());
}

function run() {
  const binPath = resolveBinaryPath();
  if (!fs.existsSync(binPath)) {
    throw new Error(`Native pkg-rust binary not found at ${binPath}`);
  }

  const result = spawnSync(binPath, process.argv.slice(2), { stdio: "inherit" });
  if (result.error) {
    throw result.error;
  }
  if (typeof result.signal === "string") {
    process.kill(process.pid, result.signal);
    return;
  }
  process.exit(result.status === null ? 1 : result.status);
}

try {
  run();
} catch (error) {
  console.error(error.message);
  process.exit(1);
}
