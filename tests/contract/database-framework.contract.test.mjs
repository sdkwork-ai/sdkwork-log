#!/usr/bin/env node
import assert from 'node:assert/strict';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { validateDatabaseFramework } from '../../../sdkwork-specs/tools/check-database-framework-standard.mjs';

const sourcePath = fileURLToPath(import.meta.url).replaceAll('\\', '/');
const isTemplateSource = sourcePath.endsWith('/sdkwork-specs/templates/tests/database-framework.contract.test.mjs');

test('database framework contract', { skip: isTemplateSource && 'template source is not an application root' }, () => {
  const result = validateDatabaseFramework(process.cwd());
  assert.equal(result.skipped, false, 'application must own database/');
  assert.equal(result.ok, true, `database framework validation failed: ${result.failures.join('; ')}`);
});
