#!/usr/bin/env node
import fs from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import path from 'node:path';

const projectDir = process.cwd();
const distDir = path.join(projectDir, 'dist');
const generatedRoot = path.join(projectDir, 'generated', 'server-openapi');
const generatedBuildScript = path.join(generatedRoot, 'custom', 'build-runtime.mjs');
const generatedDistDir = path.join(generatedRoot, 'dist');

async function main() {
  await removeDirectory(distDir);
  await requireFile(generatedBuildScript);
  runGeneratedBuild();
  await requireFile(path.join(generatedDistDir, 'index.js'));
  await requireFile(path.join(generatedDistDir, 'index.cjs'));
  await requireFile(path.join(generatedDistDir, 'index.d.ts'));
  await fs.cp(generatedDistDir, distDir, { recursive: true });
}

async function removeDirectory(target) {
  await fs.rm(target, {
    recursive: true,
    force: true,
    maxRetries: 5,
    retryDelay: 100,
  });
}

async function requireFile(filePath) {
  const stat = await fs.stat(filePath).catch(() => null);
  if (!stat?.isFile()) {
    throw new Error(`Required generated SDK build artifact is missing: ${filePath}`);
  }
}

function runGeneratedBuild() {
  const result = spawnSync(process.execPath, [generatedBuildScript], {
    cwd: generatedRoot,
    stdio: 'inherit',
  });
  if (result.error) {
    throw result.error;
  }
  if ((result.status ?? 1) !== 0) {
    throw new Error(`Generated SDK build failed with exit code ${result.status ?? 1}`);
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
