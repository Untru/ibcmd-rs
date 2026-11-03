// Execute the current pure client adapter in memory; no extension host or wiring.
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const fsp = require('node:fs/promises');
const path = require('node:path');
const os = require('node:os');
const vm = require('node:vm');
const { createHash } = require('node:crypto');

const compilerPath = process.env.STATUS_LAB_TYPESCRIPT;
if (!compilerPath || !path.isAbsolute(compilerPath)) {
  throw new Error('STATUS_LAB_TYPESCRIPT must name an existing absolute TypeScript module; no installation is attempted');
}
const ts = require(compilerPath);
const sourcePath = path.resolve(__dirname, '../../editors/vscode/src/object-status.ts');
const source = fs.readFileSync(sourcePath, 'utf8');
const code = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  fileName: sourcePath,
}).outputText;
const adapter = { exports: {} };
vm.runInThisContext(require('node:module').wrap(code), { filename: sourcePath })(
  adapter.exports, require, adapter, sourcePath, path.dirname(sourcePath),
);
const { classifyVersioned, localChanged, comparisons } = adapter.exports;
const baseline = 'b'.repeat(64), snapshot = 'd'.repeat(64);
const row = (status = 'same', changed = []) => ({
  object: 'CommonModule.Control', version: 'opaque-cache-token', status,
  changed, baseOnly: [], localOnly: [],
});
const evidence = (changed = false) => ({
  basis: 'native-config-version', object: 'CommonModule.Control',
  objectVersion: 'opaque-cache-token', baselineIdentity: baseline,
  databaseSnapshot: snapshot, conclusive: true, changed,
});
const classify = (current, reference, native, file) => classifyVersioned(
  current, reference, native, file, baseline, snapshot,
);

test('actual adapter: independent native/file matrix, including equal current bytes', (t) => {
  const states = [[false, false, 'same'], [true, false, 'database'],
    [false, true, 'file'], [true, true, 'both']];
  for (const [databaseChanged, fileChanged, expected] of states) {
    assert.equal(classify(row('same'), row('modified'), evidence(databaseChanged), fileChanged), expected);
  }
  assert.equal(classify(row('notExported')), 'databaseOnly');
  assert.equal(classify(row('notInBase')), 'fileOnly');
  t.diagnostic(`object-status.ts SHA256 ${createHash('sha256').update(source).digest('hex')}`);
});

test('historical unchanged reference is valid, not stale merely because old', () => {
  assert.equal(classify(row(), row(), evidence(false), false), 'same');
});

test('identity, coverage and cache-token mismatches stay unknown', () => {
  for (const patch of [
    { basis: 'byte-hash' }, { object: 'CommonModule.Other' },
    { objectVersion: 'another-token' }, { baselineIdentity: 'a'.repeat(64) },
    { databaseSnapshot: 'e'.repeat(64) }, { conclusive: false }, { changed: undefined },
  ]) assert.equal(classify(row(), row(), { ...evidence(), ...patch }, false), 'unknown');
  for (const version of [undefined, '', true, 1]) {
    assert.equal(classify({ ...row(), version }, { ...row(), version }, { ...evidence(), objectVersion: version }, false), 'unknown');
  }
  assert.equal(classify(row(), { ...row(), version: 'late-reference-token' }, evidence(), false), 'unknown');
  assert.equal(classify(row(), undefined, evidence(), false), 'unknown');
});

test('full-required/unmapped evidence cannot become clean', () => {
  assert.equal(classify(row(), row(), { ...evidence(), conclusive: false, changed: undefined }, false), 'unknown');
});

test('errors or malformed status rows are not successful empty comparisons', () => {
  for (const response of [{ error: { code: -32800 } }, {}, { objects: [{}] }, { objects: [row('future-status')] }]) {
    assert.throws(() => comparisons(response));
  }
});

test('actual local/reference bytes detect both even when current DB/local agree', async () => {
  // Locally retained always; no deletion/teardown, including failure paths.
  const root = await fsp.mkdtemp(path.join(os.tmpdir(), 'sql-status-client-'));
  const sourceDir = path.join(root, 'source'), baselineDir = path.join(root, 'baseline');
  await fsp.mkdir(sourceDir); await fsp.mkdir(baselineDir);
  await fsp.writeFile(path.join(sourceDir, 'module'), Buffer.from([1, 255]));
  await fsp.writeFile(path.join(baselineDir, 'module'), Buffer.from([0, 255]));
  const before = await fsp.readFile(path.join(sourceDir, 'module'));
  const current = row('same'), reference = row('modified', ['module']);
  const fileChanged = await localChanged(sourceDir, baselineDir, current, reference);
  assert.equal(fileChanged, true);
  assert.equal(classify(current, reference, evidence(true), fileChanged), 'both');
  assert.deepEqual(await fsp.readFile(path.join(sourceDir, 'module')), before);
});
