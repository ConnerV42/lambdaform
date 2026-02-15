#!/usr/bin/env node

"use strict";

const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const https = require("https");
const http = require("http");
const { createGunzip } = require("zlib");
const { pipeline } = require("stream/promises");
const tar = require("tar") !== undefined ? require("tar") : null;

const VERSION = require("./package.json").version;
const REPO = "ConnerV42/lambdaform";
const BIN_DIR = path.join(__dirname, "bin");
const BIN_PATH = path.join(BIN_DIR, process.platform === "win32" ? "lambdaform.exe" : "lambdaform");

function getPlatformKey() {
  const arch = process.arch;
  const platform = process.platform;

  if (platform === "darwin" && arch === "arm64") return "macos-aarch64";
  if (platform === "darwin" && arch === "x64") return "macos-x86_64";
  if (platform === "linux" && arch === "x64") return "linux-x86_64";
  if (platform === "linux" && arch === "arm64") return "linux-aarch64";

  throw new Error(
    `Unsupported platform: ${platform}-${arch}. ` +
    `Lambdaform supports macOS (x64/arm64) and Linux (x64/arm64). ` +
    `You can build from source: https://github.com/${REPO}`
  );
}

function fetch(url) {
  return new Promise((resolve, reject) => {
    const lib = url.startsWith("https") ? https : http;
    lib.get(url, { headers: { "User-Agent": "lambdaform-npm" } }, (res) => {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        return fetch(res.headers.location).then(resolve, reject);
      }
      if (res.statusCode !== 200) {
        return reject(new Error(`HTTP ${res.statusCode} for ${url}`));
      }
      resolve(res);
    }).on("error", reject);
  });
}

async function extractTarGz(stream, destDir) {
  // Simple tar.gz extraction without external dependencies
  const { execSync } = require("child_process");
  const tmpFile = path.join(destDir, "_download.tar.gz");

  await new Promise((resolve, reject) => {
    const out = fs.createWriteStream(tmpFile);
    stream.pipe(out);
    out.on("finish", resolve);
    out.on("error", reject);
  });

  execSync(`tar xzf "${tmpFile}" -C "${destDir}"`);
  fs.unlinkSync(tmpFile);
}

async function install() {
  const platformKey = getPlatformKey();
  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/lambdaform-${platformKey}.tar.gz`;

  console.log(`Downloading lambdaform v${VERSION} for ${platformKey}...`);

  fs.mkdirSync(BIN_DIR, { recursive: true });

  const stream = await fetch(url);
  await extractTarGz(stream, BIN_DIR);

  // Find the binary (may be named lambdaform-<platform> or just lambdaform)
  const files = fs.readdirSync(BIN_DIR);
  for (const file of files) {
    if (file.startsWith("lambdaform") && !file.endsWith(".tar.gz")) {
      const src = path.join(BIN_DIR, file);
      if (src !== BIN_PATH) {
        fs.renameSync(src, BIN_PATH);
      }
      fs.chmodSync(BIN_PATH, 0o755);
      break;
    }
  }

  if (!fs.existsSync(BIN_PATH)) {
    throw new Error("Binary not found after extraction. Please report this at " +
      `https://github.com/${REPO}/issues`);
  }

  console.log(`Installed lambdaform v${VERSION} to ${BIN_PATH}`);
}

install().catch((err) => {
  console.error(`Failed to install lambdaform: ${err.message}`);
  console.error(`\nYou can install manually from: https://github.com/${REPO}/releases`);
  process.exit(1);
});
