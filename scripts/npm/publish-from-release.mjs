#!/usr/bin/env node

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';

const assetsDir = process.env.RELEASE_ASSETS_DIR;
const tag = process.env.RELEASE_TAG;
const dryRun = process.env.DRY_RUN === '1';
const repoSlug = process.env.GITHUB_REPOSITORY || 'skrulling/unused-buddy';
const repoUrl = `https://github.com/${repoSlug}`;
const publishTarget = process.env.PUBLISH_TARGET || 'all'; // all | meta | windows | none

if (!assetsDir || !tag) {
  throw new Error('RELEASE_ASSETS_DIR and RELEASE_TAG are required.');
}

const version = tag.startsWith('v') ? tag.slice(1) : tag;
if (!/^\d+\.\d+\.\d+$/.test(version)) {
  throw new Error(`Only stable semver tags are supported. Got: ${tag}`);
}

const checksumsPath = path.join(assetsDir, 'checksums.txt');
const manifestPath = path.join(assetsDir, 'asset_manifest.json');
if (!fs.existsSync(checksumsPath) || !fs.existsSync(manifestPath)) {
  throw new Error('Missing checksums.txt or asset_manifest.json in release assets.');
}

const checksums = parseChecksums(checksumsPath);
const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));

if (manifest.version !== version) {
  throw new Error(`Manifest version ${manifest.version} does not match tag version ${version}`);
}

const publishRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'unused-buddy-npm-'));
const binaryChecksums = {};

for (const artifact of manifest.artifacts) {
  const archivePath = path.join(assetsDir, artifact.archive);
  if (!fs.existsSync(archivePath)) {
    throw new Error(`Missing archive: ${artifact.archive}`);
  }

  const expectedArchiveSum = checksums.get(artifact.archive);
  if (!expectedArchiveSum) {
    throw new Error(`Archive checksum missing in checksums.txt: ${artifact.archive}`);
  }

  const actualArchiveSum = sha256File(archivePath);
  if (actualArchiveSum !== expectedArchiveSum) {
    throw new Error(`Checksum mismatch for ${artifact.archive}`);
  }

  const extractedDir = path.join(publishRoot, `extract-${artifact.package}`);
  fs.mkdirSync(extractedDir, { recursive: true });
  extractArchive(archivePath, extractedDir);

  const srcBin = path.join(extractedDir, artifact.binary);
  if (!fs.existsSync(srcBin)) {
    throw new Error(`Expected binary not found in archive ${artifact.archive}: ${artifact.binary}`);
  }

  const pkgDir = path.join(publishRoot, artifact.package);
  const pkgBinDir = path.join(pkgDir, 'bin');
  fs.mkdirSync(pkgBinDir, { recursive: true });

  const dstBinName = artifact.os === 'win32' ? 'unused-buddy.exe' : 'unused-buddy';
  const dstBin = path.join(pkgBinDir, dstBinName);
  fs.copyFileSync(srcBin, dstBin);
  if (artifact.os !== 'win32') {
    fs.chmodSync(dstBin, 0o755);
  }

  const binaryHash = sha256File(dstBin);
  binaryChecksums[artifact.package] = binaryHash;

  const packageJson = {
    name: artifact.package,
    version,
    description: `Platform binary for unused-buddy (${artifact.os}/${artifact.cpu})`,
    keywords: ['unused-code', 'dead-code', 'cli', 'rust', 'javascript', 'typescript'],
    author: 'skrulling',
    license: 'MIT',
    homepage: repoUrl,
    repository: {
      type: 'git',
      url: `git+${repoUrl}.git`,
    },
    bugs: {
      url: `${repoUrl}/issues`,
    },
    publishConfig: {
      access: 'public',
    },
    os: [artifact.os],
    cpu: [artifact.cpu],
    files: ['bin'],
    bin: {
      'unused-buddy': artifact.os === 'win32' ? 'bin/unused-buddy.exe' : 'bin/unused-buddy',
    },
  };

  fs.writeFileSync(path.join(pkgDir, 'package.json'), `${JSON.stringify(packageJson, null, 2)}\n`);
  fs.writeFileSync(path.join(pkgDir, 'README.md'), `# ${artifact.package}\n\nBinary package for unused-buddy.\n`);

  if (publishTarget === 'all') {
    npmPublish(pkgDir);
  } else if (publishTarget === 'windows' && artifact.package === 'unused-buddy-win32-x64') {
    npmPublish(pkgDir);
  }
}

const metaDir = path.join(publishRoot, 'unused-buddy');
fs.mkdirSync(path.join(metaDir, 'bin'), { recursive: true });

const optionalDependencies = {};
for (const artifact of manifest.artifacts) {
  optionalDependencies[artifact.package] = version;
}

const metaPackageJson = {
  name: 'unused-buddy',
  version,
  description: 'Fast CLI for finding, listing, and safely removing unused JS/TS code.',
  keywords: [
    'unused-code',
    'dead-code',
    'cli',
    'static-analysis',
    'javascript',
    'typescript',
    'rust',
  ],
  author: 'skrulling',
  license: 'MIT',
  homepage: repoUrl,
  repository: {
    type: 'git',
    url: `git+${repoUrl}.git`,
  },
  bugs: {
    url: `${repoUrl}/issues`,
  },
  engines: {
    node: '>=22.14.0',
    npm: '>=11.5.1',
  },
  publishConfig: {
    access: 'public',
  },
  files: ['bin', 'install.js', 'checksums.json'],
  bin: {
    'unused-buddy': 'bin/unused-buddy.js',
  },
  scripts: {
    install: 'node install.js',
  },
  optionalDependencies,
};

fs.writeFileSync(path.join(metaDir, 'package.json'), `${JSON.stringify(metaPackageJson, null, 2)}\n`);
fs.writeFileSync(path.join(metaDir, 'checksums.json'), `${JSON.stringify(binaryChecksums, null, 2)}\n`);
fs.writeFileSync(path.join(metaDir, 'install.js'), installScript());
fs.writeFileSync(path.join(metaDir, 'bin', 'unused-buddy.js'), launcherScript());
fs.chmodSync(path.join(metaDir, 'bin', 'unused-buddy.js'), 0o755);
fs.writeFileSync(path.join(metaDir, 'README.md'), '# unused-buddy\n\nCLI binary wrapper package.\n');

if (publishTarget === 'all' || publishTarget === 'meta') {
  npmPublish(metaDir);
}

function parseChecksums(filePath) {
  const map = new Map();
  const lines = fs.readFileSync(filePath, 'utf8').split(/\r?\n/).filter(Boolean);
  for (const line of lines) {
    const match = line.match(/^([a-f0-9]{64})\s+\*?(.+)$/i);
    if (!match) continue;
    map.set(match[2].trim(), match[1].toLowerCase());
  }
  return map;
}

function sha256File(filePath) {
  const hash = crypto.createHash('sha256');
  hash.update(fs.readFileSync(filePath));
  return hash.digest('hex');
}

function extractArchive(archivePath, outDir) {
  if (archivePath.endsWith('.tar.gz')) {
    execFileSync('tar', ['-xzf', archivePath, '-C', outDir], { stdio: 'inherit' });
    return;
  }
  if (archivePath.endsWith('.zip')) {
    execFileSync('unzip', ['-q', archivePath, '-d', outDir], { stdio: 'inherit' });
    return;
  }
  throw new Error(`Unsupported archive format: ${archivePath}`);
}

function npmPublish(pkgDir) {
  const args = ['publish', '--access', 'public'];
  const shouldUseProvenance =
    process.env.FORCE_PROVENANCE === '1' ||
    (process.env.GITHUB_ACTIONS === 'true' && process.env.ACTIONS_ID_TOKEN_REQUEST_URL);
  if (shouldUseProvenance) {
    args.push('--provenance');
  }
  if (dryRun) {
    args.push('--dry-run');
  }
  execFileSync('npm', args, { cwd: pkgDir, stdio: 'inherit' });
}

function launcherScript() {
  return `#!/usr/bin/env node
const fs = require('node:fs');
const path = require('node:path');
const cp = require('node:child_process');

const mapping = {
  'darwin:arm64': { pkg: 'unused-buddy-darwin-arm64', bin: 'bin/unused-buddy' },
  'linux:arm64': { pkg: 'unused-buddy-linux-arm64-gnu', bin: 'bin/unused-buddy' },
  'linux:x64': { pkg: 'unused-buddy-linux-x64-gnu', bin: 'bin/unused-buddy' },
  'win32:x64': { pkg: 'unused-buddy-win32-x64', bin: 'bin/unused-buddy.exe' },
};

const key = process.platform + ':' + process.arch;
const target = mapping[key];
if (!target) {
  console.error('unsupported platform for unused-buddy:', key);
  process.exit(1);
}

let packageJsonPath;
try {
  packageJsonPath = require.resolve(target.pkg + '/package.json');
} catch (error) {
  console.error('missing optional dependency for platform:', target.pkg);
  console.error('try reinstalling unused-buddy or use a supported platform');
  process.exit(1);
}

const binaryPath = path.join(path.dirname(packageJsonPath), target.bin);
if (!fs.existsSync(binaryPath)) {
  console.error('platform binary not found:', binaryPath);
  process.exit(1);
}

const result = cp.spawnSync(binaryPath, process.argv.slice(2), { stdio: 'inherit' });
if (typeof result.status === 'number') {
  process.exit(result.status);
}
process.exit(1);
`;
}

function installScript() {
  return `#!/usr/bin/env node
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');

const mapping = {
  'darwin:arm64': { pkg: 'unused-buddy-darwin-arm64', bin: 'bin/unused-buddy' },
  'linux:arm64': { pkg: 'unused-buddy-linux-arm64-gnu', bin: 'bin/unused-buddy' },
  'linux:x64': { pkg: 'unused-buddy-linux-x64-gnu', bin: 'bin/unused-buddy' },
  'win32:x64': { pkg: 'unused-buddy-win32-x64', bin: 'bin/unused-buddy.exe' },
};

const key = process.platform + ':' + process.arch;
const target = mapping[key];
if (!target) {
  console.error('[unused-buddy] unsupported platform:', key);
  process.exit(1);
}

const checksums = require('./checksums.json');
const expected = checksums[target.pkg];
if (!expected) {
  console.error('[unused-buddy] missing checksum entry for', target.pkg);
  process.exit(1);
}

let packageJsonPath;
try {
  packageJsonPath = require.resolve(target.pkg + '/package.json');
} catch (error) {
  console.error('[unused-buddy] missing platform package', target.pkg);
  console.error('[unused-buddy] trusted publishing likely incomplete for this release.');
  process.exit(1);
}

const binaryPath = path.join(path.dirname(packageJsonPath), target.bin);
if (!fs.existsSync(binaryPath)) {
  console.error('[unused-buddy] binary missing at', binaryPath);
  process.exit(1);
}

const hash = crypto.createHash('sha256').update(fs.readFileSync(binaryPath)).digest('hex');
if (hash !== expected) {
  console.error('[unused-buddy] checksum verification failed for', target.pkg);
  process.exit(1);
}
`;
}
