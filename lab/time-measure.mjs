// Times the page's "Measure over 200 seeds" (`Lab.measure`) in headless Chromium, with and
// without the CPU slowdown the site's performance check applies (christianjmills
// `tools/check-perf.ts`: a 4x slowdown through the DevTools protocol). For every recipe in
// lab/recipes/ and lab/recipes/own/ it prints, at each slowdown:
//
//   - the first build with every frame kept (`new Lab`), which is what a mount waits for;
//   - all 200 seeds in one call;
//   - the longest of the 20 calls when the 200 seeds are taken ten at a time, which is how
//     long the page is unresponsive if it measures in slices.
//
// It also prints how long the WebAssembly module takes to compile and start. Network
// throttling is left out: nothing here is fetched after the module has loaded.
//
//   node lab/time-measure.mjs [--out FILE]     needs lab/target/web/ (node lab/measure-size.mjs)
//
// The browser is $CHROME, or Playwright's headless shell in ~/.cache/ms-playwright, or
// google-chrome. It is driven over the DevTools protocol directly, so this repo still has no
// dependencies. Times vary from run to run and machine to machine: they are a reading, not a
// check, and nothing here fails on a slow one.
import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { homedir, tmpdir } from 'node:os';
import { join } from 'node:path';

const here = import.meta.dirname;
const web = join(here, 'target', 'web');
const SEEDS = 200;
const SLICE = 10;
const RATES = [1, 4];
const out = process.argv.includes('--out') ? process.argv[process.argv.indexOf('--out') + 1] : null;

function findChrome() {
  if (process.env.CHROME) return process.env.CHROME;
  const cache = join(homedir(), '.cache', 'ms-playwright');
  if (existsSync(cache)) {
    const shells = readdirSync(cache).filter((d) => d.startsWith('chromium_headless_shell-')).sort();
    for (const d of shells.reverse()) {
      const p = join(cache, d, 'chrome-headless-shell-linux64', 'chrome-headless-shell');
      if (existsSync(p)) return p;
    }
  }
  return 'google-chrome';
}

const list = (sub) => readdirSync(join(here, sub)).filter((f) => f.endsWith('.json')).sort().map((f) => join(here, sub, f));
const recipes = [...list('recipes'), ...list(join('recipes', 'own'))].map((file) => ({ id: file.split('/').pop().replace(/\.json$/, ''), text: readFileSync(file, 'utf8') }));

const types = { '.js': 'text/javascript', '.wasm': 'application/wasm' };
const server = createServer((req, res) => {
  const name = req.url.split('?')[0].slice(1);
  const file = join(web, name);
  if (name === '') return res.writeHead(200, { 'content-type': 'text/html' }).end('<!doctype html><title>map lab timing</title>');
  if (name.includes('/') || !existsSync(file)) return res.writeHead(404).end();
  res.writeHead(200, { 'content-type': types[name.slice(name.lastIndexOf('.'))] ?? 'application/octet-stream' }).end(readFileSync(file));
});
await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
const origin = `http://127.0.0.1:${server.address().port}`;

const profile = mkdtempSync(join(tmpdir(), 'map-lab-timing-'));
const chrome = spawn(findChrome(), ['--headless', '--remote-debugging-port=0', `--user-data-dir=${profile}`, '--no-first-run', 'about:blank'], { stdio: ['ignore', 'ignore', 'pipe'] });
const quit = () => {
  chrome.kill();
  server.close();
  rmSync(profile, { recursive: true, force: true });
};
try {
  const endpoint = await new Promise((ok, no) => {
    let err = '';
    chrome.stderr.on('data', (d) => {
      err += d;
      const m = err.match(/DevTools listening on (ws:\/\/\S+)/);
      if (m) ok(m[1]);
    });
    chrome.on('error', no);
    chrome.on('exit', () => no(new Error(`the browser exited before it was ready:\n${err}`)));
  });
  const ws = new WebSocket(endpoint);
  await new Promise((ok, no) => {
    ws.onopen = ok;
    ws.onerror = () => no(new Error('could not reach the browser'));
  });
  let next = 0;
  const waiting = new Map();
  ws.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.id && waiting.has(m.id)) {
      const { ok, no } = waiting.get(m.id);
      waiting.delete(m.id);
      m.error ? no(new Error(m.error.message)) : ok(m.result);
    }
  };
  const send = (method, params = {}, sessionId) =>
    new Promise((ok, no) => {
      const id = ++next;
      waiting.set(id, { ok, no });
      ws.send(JSON.stringify({ id, method, params, sessionId }));
    });

  const { targetId } = await send('Target.createTarget', { url: origin + '/' });
  const { sessionId } = await send('Target.attachToTarget', { targetId, flatten: true });
  const evaluate = async (expression) => {
    const r = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true }, sessionId);
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
    return r.result.value;
  };

  // Runs in the page. Each recipe is measured three times and the median kept.
  const inPage = async (recipes, SEEDS, SLICE) => {
    const t0 = performance.now();
    const glue = await import(`/map_lab_wasm.js?${Math.random()}`);
    await glue.default();
    const start = performance.now() - t0;
    const median = (f) => [f(), f(), f()].sort((a, b) => a - b)[1];
    const rows = recipes.map(({ id, text }) => {
      const build = median(() => {
        const t = performance.now();
        new glue.Lab(text).free();
        return performance.now() - t;
      });
      const whole = median(() => {
        const lab = new glue.Lab(text);
        const t = performance.now();
        lab.measure(SEEDS);
        const ms = performance.now() - t;
        lab.free();
        return ms;
      });
      const slice = median(() => {
        const lab = new glue.Lab(text);
        let longest = 0;
        for (let n = 0; n < SEEDS; n += SLICE) {
          const t = performance.now();
          lab.measure(SLICE);
          longest = Math.max(longest, performance.now() - t);
        }
        lab.free();
        return longest;
      });
      return { id, build, whole, slice };
    });
    return { start, rows };
  };

  const version = (await send('Browser.getVersion')).product;
  const result = { browser: version, seeds: SEEDS, slice: SLICE, rates: {} };
  for (const rate of RATES) {
    await send('Emulation.setCPUThrottlingRate', { rate }, sessionId);
    result.rates[rate] = await evaluate(`(${inPage})(${JSON.stringify(recipes)}, ${SEEDS}, ${SLICE})`);
  }

  const ms = (x) => `${x.toFixed(x < 10 ? 1 : 0)}`.padStart(7);
  console.log(`${version}; ${SEEDS} seeds a recipe, slices of ${SLICE}; milliseconds, the median of three runs`);
  console.log(`module compile and start: ${RATES.map((r) => `${result.rates[r].start.toFixed(0)} ms at ${r}x`).join(', ')}`);
  console.log(`${'recipe'.padEnd(22)}${RATES.map((r) => `  | ${r}x: build  200 seeds  slice`).join('')}`);
  recipes.forEach(({ id }, n) => {
    console.log(`${id.padEnd(22)}${RATES.map((r) => { const x = result.rates[r].rows[n]; return `  | ${ms(x.build)}  ${ms(x.whole)}  ${ms(x.slice)} `; }).join('')}`);
  });
  for (const r of RATES) {
    const rows = result.rates[r].rows;
    const worst = (k) => rows.reduce((a, b) => (b[k] > a[k] ? b : a));
    console.log(`at ${r}x the slowest: build ${worst('build').id} ${worst('build').build.toFixed(0)} ms; 200 seeds ${worst('whole').id} ${worst('whole').whole.toFixed(0)} ms; one slice of ${SLICE} ${worst('slice').id} ${worst('slice').slice.toFixed(0)} ms`);
  }
  if (out) writeFileSync(out, JSON.stringify(result, null, 2) + '\n');
  ws.close();
} finally {
  quit();
}
