// Production application comparison, not a language or isolated atom-runtime microbenchmark.
import { chromium } from '@playwright/test';
import { readFile, writeFile, mkdir, readdir } from 'node:fs/promises';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { gzipSync } from 'node:zlib';
import os from 'node:os';
import { execFileSync } from 'node:child_process';
import assert from 'node:assert/strict';
import { serve } from './server.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));
const options = Object.fromEntries(process.argv.slice(2).map(arg => {
  const equal = arg.indexOf('=');
  assert(equal > 2, 'Arguments use --name=value');
  return [arg.slice(2, equal), arg.slice(equal + 1)];
}));
assert(options['ts-dir'], 'Pass --ts-dir=/path/to/jotai-holo/.spreadsheet-performance');
const tsDir = resolve(options['ts-dir']);
const tsRoot = dirname(tsDir);
const rustDir = resolve(options['rust-dir'] || resolve(root, 'target/spreadsheet-comparison/rust'));
const output = resolve(options.out || resolve(root, 'benchmarks/spreadsheet/results/latest.json'));
const samples = Number(options.samples || 30);
const coldSamples = Number(options.cold || 10);
assert(Number.isInteger(samples) && samples >= 5 && Number.isInteger(coldSamples) && coldSamples >= 2);
const warmups = 5;
const hash = data => createHash('sha256').update(data).digest('hex');
const git = (cwd, args) => execFileSync('git', args, { cwd, encoding: 'utf8' }).trim();
const sources = {};
for (const file of ['penguins.csv', 'titanic.csv']) {
  const ts = await readFile(resolve(tsRoot, 'examples/spreadsheet/data', file));
  const rust = await readFile(resolve(root, 'examples/yew-spreadsheet/data', file));
  assert.equal(hash(ts), hash(rust), `Different input bytes: ${file}`);
  sources[file] = { sha256: hash(ts), bytes: ts.length };
}

async function assets(directory) {
  const files = [];
  async function walk(path) {
    for (const item of await readdir(path, { withFileTypes: true })) {
      const full = resolve(path, item.name);
      if (item.isDirectory()) await walk(full);
      else if (/\.(?:js|wasm|css|html)$/.test(item.name)) {
        const content = await readFile(full);
        files.push({ name: full.slice(directory.length + 1), bytes: content.length, gzipBytes: gzipSync(content).length, sha256: hash(content) });
      }
    }
  }
  await walk(directory);
  return files;
}

// The same hooks are installed before either application starts. Input value writes plus
// mutation observation give a browser-local event→DOM endpoint, without CDP round trips.
function instrument() {
  let pending;
  const numeric = value => Number(value.replace(/[,\s\u00a0\u202f]/g, ''));
  const state = window.__sheetBenchmark = { giftReady: null };
  const match = spec => {
    const input = document.querySelector(`[data-address="${spec.address}"]`);
    if (!input || (spec.readonly && !input.readOnly)) return false;
    return spec.numeric ? Math.abs(numeric(input.value) - spec.expected) < 0.0051 : input.value === spec.expected;
  };
  const check = () => {
    if (state.giftReady === null && match({ address: 'D5', expected: 4136, numeric: true })) state.giftReady = performance.now();
    if (pending && match(pending.spec)) {
      const current = pending;
      pending = undefined;
      clearTimeout(current.timer);
      current.resolve(performance.now() - current.start);
    }
  };
  const descriptor = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value');
  Object.defineProperty(HTMLInputElement.prototype, 'value', {
    ...descriptor,
    set(value) { descriptor.set.call(this, value); if (pending) check(); },
  });
  new MutationObserver(check).observe(document, { subtree: true, childList: true, attributes: true, characterData: true });
  state.measure = (spec, trigger) => new Promise((resolve, reject) => {
    if (pending) { reject(new Error('Overlapping measurements')); return; }
    const timer = setTimeout(() => { pending = undefined; reject(new Error(`DOM timeout: ${JSON.stringify(spec)}`)); }, 10_000);
    pending = { spec, start: performance.now(), timer, resolve };
    if (trigger.type === 'tab') {
      const button = [...document.querySelectorAll('button[data-sheet-id]')].find(button => button.textContent.trim().replace(/[ ·!]+$/, '') === trigger.name);
      if (!button) { clearTimeout(timer); pending = undefined; reject(new Error(`Missing tab ${trigger.name}`)); return; }
      button.click();
    } else {
      document.querySelector(`[data-address="${trigger.address}"]`).dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', code: 'Enter', bubbles: true, cancelable: true }));
    }
    check();
  });
}

const implementations = {
  ts: { directory: tsDir, saved: '.sheet-status[data-state="saved"]', go: '.sheet-camera form input', goButton: '.sheet-camera form button' },
  rust: { directory: rustDir, saved: '.save-status.saved', go: '.go-form input', goButton: '.go-form button' },
};
const result = {
  generatedAt: new Date().toISOString(),
  environment: { platform: os.platform(), release: os.release(), arch: os.arch(), cpu: os.cpus()[0]?.model, logicalCpus: os.cpus().length, node: process.version, viewport: { width: 1100, height: 900 } },
  method: { samples, coldSamples, warmups, browser: 'headless Chromium, one process, sequential work, fresh context for cold runs', endpoint: 'browser performance.now: event dispatch → matching DOM value; not time to paint', persistence: 'native application storage; wait for saved state outside measured intervals', warmOrder: 'TS, Rust, Rust, TS blocks; fresh context and five discarded warmups per block; samples split between blocks', builds: 'Vite production; Trunk --release (wasm-opt default)' },
  sources,
  versions: { ts: { commit: git(tsRoot, ['rev-parse', 'HEAD']), dirty: Boolean(git(tsRoot, ['status', '--porcelain'])) }, rust: { commit: git(root, ['rev-parse', 'HEAD']), dirty: Boolean(git(root, ['status', '--porcelain'])) } },
  apps: {},
};
const servers = [];
let browser;
try {
  for (const [name, implementation] of Object.entries(implementations)) {
    const server = await serve(implementation.directory);
    servers.push(server);
    implementation.url = server.url;
    result.apps[name] = { assets: await assets(implementation.directory), timings: {} };
  }
  browser = await chromium.launch();
  result.environment.chromium = browser.version();
  const record = (name, metric, value) => (result.apps[name].timings[metric] ??= []).push(value);
  async function pageFor(name) {
    const context = await browser.newContext({ viewport: result.environment.viewport, reducedMotion: 'reduce' });
    await context.addInitScript(instrument);
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.goto(implementations[name].url);
    await page.waitForFunction(() => window.__sheetBenchmark.giftReady !== null);
    await page.locator(implementations[name].saved).waitFor();
    return { context, page, errors };
  }
  async function switchTab(page, name, tab) {
    const header = tab === 'Penguins' ? 'species' : 'survived';
    const elapsed = await page.evaluate(({ header, tab }) => window.__sheetBenchmark.measure({ address: 'A1', expected: header }, { type: 'tab', name: tab }), { header, tab });
    await page.locator(implementations[name].saved).waitFor();
    return elapsed;
  }
  // Balance implementation order between rounds. Fresh storage contexts, not fresh OS/browser processes.
  for (let run = 0; run < coldSamples; run++) {
    for (const name of run % 2 ? ['rust', 'ts'] : ['ts', 'rust']) {
      const { context, page, errors } = await pageFor(name);
      record(name, 'open_gift_book', await page.evaluate(() => window.__sheetBenchmark.giftReady));
      record(name, 'first_open_penguins', await switchTab(page, name, 'Penguins'));
      record(name, 'first_open_titanic', await switchTab(page, name, 'Titanic'));
      assert.deepEqual(errors, [], `${name} cold browser errors`);
      await context.close();
    }
    console.log(`cold round ${run + 1}/${coldSamples}`);
  }
  for (const [block, name] of [[0, 'ts'], [0, 'rust'], [1, 'rust'], [1, 'ts']]) {
    const blockSamples = block === 0 ? Math.floor(samples / 2) : samples - Math.floor(samples / 2);
    const { context, page, errors } = await pageFor(name);
    for (let run = 0; run < blockSamples + warmups; run++) {
      for (const tab of ['Penguins', 'Titanic']) {
        const elapsed = await switchTab(page, name, tab);
        if (run >= warmups) record(name, `warm_switch_${tab.toLowerCase()}`, elapsed);
      }
    }
    console.log(`${name}: warm switches complete`);
    const cell = address => page.locator(`[data-address="${address}"]`);
    const numericValue = async address => Number((await cell(address).inputValue()).replace(/[,\s\u00a0\u202f]/g, ''));
    async function go(address) {
      await page.locator(implementations[name].go).fill(address);
      await page.locator(implementations[name].goButton).click();
      await cell(address).waitFor();
      await page.locator(implementations[name].saved).waitFor();
    }
    async function draft(address, value) {
      await cell(address).dblclick();
      await cell(address).fill(value);
    }
    const fixtures = [
      { tab: 'Penguins', source: 'C2', output: 'D2', formula: '=SUM(C2:C345)' },
      { tab: 'Titanic', source: 'G2', output: 'F2', formula: '=SUM(G2:G892)' },
    ];
    for (const fixture of fixtures) {
      await switchTab(page, name, fixture.tab);
      await go(fixture.source);
      for (let run = 0; run < blockSamples + warmups; run++) {
        const next = 1000 + run;
        await go(fixture.source);
        await draft(fixture.source, `${next}.000`);
        const elapsed = await page.evaluate(({ source, next }) => window.__sheetBenchmark.measure({ address: source, expected: next, numeric: true, readonly: true }, { type: 'commit', address: source }), { source: fixture.source, next });
        if (run >= warmups) record(name, `plain_edit_${fixture.tab.toLowerCase()}`, elapsed);
        await page.locator(implementations[name].saved).waitFor();
      }
      console.log(`${name}: ${fixture.tab} plain edits complete`);
      await go(fixture.output);
      await draft(fixture.output, fixture.formula);
      await cell(fixture.output).press('Enter');
      await page.locator(implementations[name].saved).waitFor();
      await go(fixture.source);
      for (let run = 0; run < blockSamples + warmups; run++) {
        await go(fixture.source);
        const previous = await numericValue(fixture.source);
        const total = await numericValue(fixture.output);
        const next = 2000 + run;
        await draft(fixture.source, String(next));
        const elapsed = await page.evaluate(({ source, output, expected }) => window.__sheetBenchmark.measure({ address: output, expected, numeric: true }, { type: 'commit', address: source }), { source: fixture.source, output: fixture.output, expected: total + next - previous });
        if (run >= warmups) record(name, `sum_edit_${fixture.tab.toLowerCase()}`, elapsed);
        await page.locator(implementations[name].saved).waitFor();
      }
      console.log(`${name}: ${fixture.tab} SUM edits complete`);
    }
    assert.deepEqual(errors, [], `${name} browser errors`);
    await context.close();
  }
  const percentile = (values, p) => [...values].sort((a, b) => a - b)[Math.ceil(values.length * p) - 1];
  for (const app of Object.values(result.apps)) {
    app.summary = Object.fromEntries(Object.entries(app.timings).map(([metric, values]) => [metric, { n: values.length, medianMs: percentile(values, .5), p95Ms: percentile(values, .95), minMs: Math.min(...values), maxMs: Math.max(...values) }]));
  }
  await mkdir(dirname(output), { recursive: true });
  await writeFile(output, JSON.stringify(result, null, 2) + '\n');
  console.table(Object.fromEntries(Object.keys(result.apps.ts.summary).map(metric => [metric, { tsMedian: result.apps.ts.summary[metric].medianMs.toFixed(2), rustMedian: result.apps.rust.summary[metric].medianMs.toFixed(2), tsP95: result.apps.ts.summary[metric].p95Ms.toFixed(2), rustP95: result.apps.rust.summary[metric].p95Ms.toFixed(2) }])));
  console.log(output);
} finally {
  await browser?.close();
  await Promise.all(servers.map(server => server.close()));
}
