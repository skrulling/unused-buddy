#!/usr/bin/env node

import fs from 'node:fs';
import { execFileSync } from 'node:child_process';
import path from 'node:path';

const args = process.argv.slice(2);
const dryRun = args.includes('--dry-run');
const skipTests = args.includes('--skip-tests');
const versionArg = args.find((a) => !a.startsWith('-'));

if (args.includes('--help')) {
  printUsage();
  process.exit(0);
}

if (!versionArg) {
  printUsage();
  process.exit(1);
}

const version = versionArg.trim();
if (!/^\d+\.\d+\.\d+$/.test(version)) {
  fail(`invalid version '${version}'. Expected stable semver like 0.2.0`);
}

const tag = `v${version}`;

run('git', ['rev-parse', '--is-inside-work-tree']);
ensureCleanWorkingTree();
ensureTagDoesNotExist(tag);

const branch = capture('git', ['branch', '--show-current']).trim();
if (!branch) {
  fail('could not determine current branch');
}

const repoRoot = capture('git', ['rev-parse', '--show-toplevel']).trim();
const cargoTomlPath = path.join(repoRoot, 'Cargo.toml');
if (!fs.existsSync(cargoTomlPath)) {
  fail('Cargo.toml not found at repository root');
}

const currentVersion = readPackageVersion(cargoTomlPath);
const needsVersionBump = currentVersion !== version;

if (!skipTests) {
  run('cargo', ['test', '-q'], { dryRun });
}

if (needsVersionBump) {
  const updated = bumpCargoVersion(cargoTomlPath, version);
  if (!updated) {
    fail('failed to update Cargo.toml version');
  }
} else {
  console.log(`Cargo.toml already at ${version}; continuing release without version bump.`);
}

try {
  if (needsVersionBump) {
    run('git', ['add', 'Cargo.toml'], { dryRun });
    run('git', ['commit', '-m', `release: ${tag}`], { dryRun });
  } else {
    run('git', ['commit', '--allow-empty', '-m', `release: ${tag}`], { dryRun });
  }
  run('git', ['tag', tag], { dryRun });
  run('git', ['push', 'origin', branch], { dryRun });
  run('git', ['push', 'origin', tag], { dryRun });
} catch (error) {
  if (!dryRun) {
    // Leave file as-is so user can inspect/fix if needed.
  }
  throw error;
}

if (dryRun) {
  console.log('\nDry run complete. No files or refs changed.');
} else {
  console.log(`\nRelease prepared and pushed: ${tag}`);
}

function printUsage() {
  console.log(`Usage:\n  npm run release -- <version> [--dry-run] [--skip-tests]\n\nExamples:\n  npm run release -- 0.2.0\n  npm run release -- 0.2.0 --dry-run`);
}

function ensureCleanWorkingTree() {
  const out = capture('git', ['status', '--porcelain']);
  if (out.trim().length > 0) {
    fail('working tree is not clean. Commit or stash changes before releasing.');
  }
}

function ensureTagDoesNotExist(tag) {
  const localTags = capture('git', ['tag', '--list', tag]).trim();
  if (localTags === tag) {
    fail(`tag already exists locally: ${tag}`);
  }

  const remote = capture('git', ['ls-remote', '--tags', 'origin', tag]).trim();
  if (remote.length > 0) {
    fail(`tag already exists on remote: ${tag}`);
  }
}

function readPackageVersion(cargoTomlPath) {
  const content = fs.readFileSync(cargoTomlPath, 'utf8');
  const packageBlock = content.match(/\[package\][\s\S]*?(?:\n\[|$)/);
  if (!packageBlock) fail('could not locate [package] section in Cargo.toml');
  const versionLine = packageBlock[0].match(/^version\s*=\s*"([^"]+)"/m);
  if (!versionLine) fail('could not locate package version in Cargo.toml');
  return versionLine[1];
}

function bumpCargoVersion(cargoTomlPath, version) {
  const content = fs.readFileSync(cargoTomlPath, 'utf8');
  const packageMatch = content.match(/\[package\][\s\S]*?(?=\n\[|$)/);
  if (!packageMatch) return false;

  const packageBlock = packageMatch[0];
  if (!/^version\s*=\s*"[^"]+"/m.test(packageBlock)) {
    return false;
  }

  const updatedBlock = packageBlock.replace(
    /^version\s*=\s*"[^"]+"/m,
    `version = "${version}"`
  );
  const updatedContent = content.replace(packageBlock, updatedBlock);

  if (!dryRun) {
    fs.writeFileSync(cargoTomlPath, updatedContent);
  }

  return true;
}

function run(cmd, cmdArgs, options = {}) {
  if (options.dryRun) {
    console.log(`[dry-run] ${cmd} ${cmdArgs.join(' ')}`);
    return;
  }
  execFileSync(cmd, cmdArgs, { stdio: 'inherit' });
}

function capture(cmd, cmdArgs) {
  return execFileSync(cmd, cmdArgs, { encoding: 'utf8' });
}

function fail(message) {
  console.error(`release error: ${message}`);
  process.exit(1);
}
