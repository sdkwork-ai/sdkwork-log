#!/usr/bin/env node
// Regenerates the sdkwork-log-backend-sdk TypeScript package from the
// committed OpenAPI authority (apis/backend-api/log/openapi.json).
//
// Usage: node bin/generate-sdk.mjs [--language typescript]
import { copyFileSync, existsSync, rmSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const familyRoot = path.resolve(__dirname, '..');
const workspaceRoot = path.resolve(familyRoot, '..', '..');
const authoritySpec = path.resolve(
  workspaceRoot,
  'apis',
  'backend-api',
  'log',
  'openapi.json',
);
const familyInputSpec = path.join(familyRoot, 'openapi', 'sdkwork-log-backend-sdk.openapi.json');
const sdkGeneratorCli = path.resolve(workspaceRoot, '../sdkwork-sdk-generator/bin/sdkgen.js');
const sdkFamily = 'sdkwork-log-backend-sdk';
const baseUrl = 'http://localhost:18081';
const apiPrefix = '/backend/v3/api';
const description = 'SDKWork Log backend API SDK';

const languages = parseLanguages(process.argv.slice(2));
syncFamilyOpenApiSnapshots();
for (const language of languages) {
  runLanguage(language);
}

function parseLanguages(argv) {
  const selected = [];
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--language' || arg === '-l') {
      const value = argv[index + 1];
      if (!value || value.startsWith('-')) {
        throw new Error(`${arg} requires a language value`);
      }
      selected.push(...value.split(','));
      index += 1;
      continue;
    }
    throw new Error(`Unsupported SDK generation option: ${arg}`);
  }
  return selected.length === 0 ? ['typescript'] : selected;
}

function syncFamilyOpenApiSnapshots() {
  if (!existsSync(authoritySpec)) {
    throw new Error(`OpenAPI authority not found: ${authoritySpec}`);
  }
  copyFileSync(authoritySpec, familyInputSpec);
  console.log(`Synced ${familyInputSpec}`);
}

function runLanguage(language) {
  const output = path.join(familyRoot, `${sdkFamily}-${language}`, 'generated', 'server-openapi');
  rmSync(output, { recursive: true, force: true });
  const args = [
    sdkGeneratorCli,
    'generate',
    '-i', familyInputSpec,
    '-o', output,
    '-n', sdkFamily,
    '-t', 'backend',
    '-l', language,
    '--base-url', baseUrl,
    '--api-prefix', apiPrefix,
    '--package-name', '@sdkwork/log-backend-sdk',
    '--description', description,
    '--fixed-sdk-version', '0.1.0',
    '--no-sync-published-version',
    '--standard-profile', 'sdkwork-v3',
  ];
  const result = spawnSync(process.execPath, args, {
    cwd: workspaceRoot,
    stdio: 'inherit',
    env: { ...process.env, MSYS_NO_PATHCONV: '1' },
  });
  if (result.error) {
    throw result.error;
  }
  if ((result.status ?? 1) !== 0) {
    process.exit(result.status ?? 1);
  }
}
