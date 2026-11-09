// The deliverable's build: stage the bundle in dist/bundle/, resolve the page's passage-id
// citations into the site's locators, regenerate and check the page's own measurements, render the
// no-JavaScript fallback there from the same code the interactive runs, stamp the commit it was
// built from, and optionally hand the bundle to the site's publish command.
//
//   node build.mjs                     stage the bundle only
//   node build.mjs --publish <args>    stage, then run the site's publish command with <args>
//
// The publish command belongs to the site, which owns the deliverable format. It is run from a
// checkout of cj-mills/christianjmills: $CHRISTIANJMILLS_SITE, or ../christianjmills beside this
// repo. That checkout's spec verifies the bundle and its lockfile is the one pinned.
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cpSync, existsSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { crc32, deflateSync } from 'node:zlib';
import { applyCitations } from './apply-citations.mjs';
import { generate, openShare } from './deliverable/assets/caves.js';
import { checkOwnRuns, measure } from './measure.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const source = join(here, 'deliverable');
const bundle = join(here, 'dist/bundle');
const args = process.argv.slice(2);
const publish = args.indexOf('--publish');
const publishArgs = publish === -1 ? [] : args.slice(publish + 1);

const git = (...a) => execFileSync('git', a, { cwd: here, encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'] }).trim();
let commit;
try {
  commit = execFileSync('git', ['rev-parse', '--verify', '--quiet', 'HEAD'], { cwd: here, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
} catch {
  commit = undefined; // no commit yet
}
const clean = commit !== undefined && git('status', '--porcelain') === '';

// A bundle published to the real bucket names the commit it was built from, so that commit must
// be clean and on GitHub. A local stand-in publish may come from an unpushed or dirty tree.
if (publishArgs.includes('--r2')) {
  if (!clean) throw new Error('commit your changes first: a published bundle names the commit it was built from');
  if (git('branch', '--remotes', '--contains', commit) === '') throw new Error(`push ${commit} first: the bundle names it`);
}

rmSync(bundle, { recursive: true, force: true });
cpSync(source, bundle, { recursive: true });
const manifestPath = join(bundle, 'manifest.json');
const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));

// The page cites passages by id ({% cite id="…" /%}). source-library's `slib trace` resolved them
// into the site's locators in the committed citations file; applying it is plain string splicing,
// refused if the page changed since it was traced. The citations file stays out of the bundle.
const entry = join(source, manifest.content.entry);
const citationsFile = entry.replace(/\.mdoc$/, '.citations.json');
const page = readFileSync(entry);
writeFileSync(join(bundle, manifest.content.entry), applyCitations(page, JSON.parse(readFileSync(citationsFile, 'utf8'))));
rmSync(join(bundle, manifest.content.entry.replace(/\.mdoc$/, '.citations.json')));

// The data for the page's "how this was checked" note, written by `slib note` from the private
// evidence: it must describe this exact page, and a published bundle may carry no open flags.
const checkingFile = entry.replace(/\.mdoc$/, '.checking.json');
const checking = JSON.parse(readFileSync(checkingFile, 'utf8'));
rmSync(join(bundle, manifest.content.entry.replace(/\.mdoc$/, '.checking.json')));
if (checking.format !== 'slib-checking@1') throw new Error(`${checkingFile}: not a slib-checking@1 file`);
if (checking.source_sha256 !== createHash('sha256').update(page).digest('hex'))
  throw new Error(`${checkingFile} describes another version of the page; run \`slib note\` on it`);
if (checking.open.length && publishArgs.includes('--r2'))
  throw new Error(`open flags: ${checking.open.map((o) => `${o.claim ? `claim ${o.claim}` : 'page'} ${o.state}`).join(', ')}`);

// Every number the page marks {% own-run values="…" /%} is regenerated here and must be the one
// its sentence states.
const measured = measure();
const runProblems = checkOwnRuns(page.toString('utf8'), measured);
if (runProblems.length) throw new Error(`the page's own numbers don't match measure.mjs:\n  ${runProblems.join('\n  ')}`);
const ownRuns = (page.toString('utf8').match(/\{%\s*own-run\b/g) ?? []).length;
if (ownRuns !== checking.claims.own_run) throw new Error(`the page has ${ownRuns} own-run claims, the checking note ${checking.claims.own_run}`);

// The manifest carries the note's facts; the site words and places it.
const implementations = { 'claude-tier': 'Claude tier' };
manifest.checked = {
  claims: checking.claims.cited,
  supported: checking.claims.verified,
  measured: ownRuns,
  verifiers: checking.verifiers.map((v) => ({
    name: `${v.contract}, ${implementations[v.implementation] ?? v.implementation}`,
    version: v.version,
    ...(v.model ? { model: v.model } : {}),
  })),
  lastVerified: checking.last_verified,
  openFlags: checking.open.length,
};
console.log(`checked: ${manifest.checked.supported} of ${manifest.checked.claims} cited claims supported (last ${manifest.checked.lastVerified}); ${ownRuns} own-run claims, ${Object.keys(measured).length} values regenerated; ${manifest.checked.openFlags} open flags`);
if (clean) manifest.code.commit = commit;
else console.warn('working tree has changes: the bundle names no commit');
writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + '\n');

const WIDTH = 80;
const HEIGHT = 50;
const SCALE = 4;
const GAP = 16;
const GENERATIONS = [0, 3, 10];
const ROCK = [0x3a, 0x3f, 0x47];
const OPEN = [0xe9, 0xe4, 0xd8];
const GAP_COLOUR = [0xff, 0xff, 0xff];

function png(width, height, rgb) {
  const chunk = (type, data) => {
    const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
    const out = Buffer.alloc(body.length + 8);
    out.writeUInt32BE(data.length, 0);
    body.copy(out, 4);
    out.writeUInt32BE(crc32(body), body.length + 4);
    return out;
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr.set([8, 2, 0, 0, 0], 8); // 8-bit RGB
  const raw = Buffer.alloc((width * 3 + 1) * height);
  for (let y = 0; y < height; y++) rgb.copy(raw, y * (width * 3 + 1) + 1, y * width * 3, (y + 1) * width * 3);
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

const panelW = WIDTH * SCALE;
const outW = GENERATIONS.length * panelW + (GENERATIONS.length - 1) * GAP;
const outH = HEIGHT * SCALE;
const rgb = Buffer.alloc(outW * outH * 3);
for (let p = 0; p < outW * outH; p++) rgb.set(GAP_COLOUR, p * 3);

GENERATIONS.forEach((iterations, panel) => {
  const grid = generate({ width: WIDTH, height: HEIGHT, rockPercent: 55, seed: 1, iterations });
  console.log(`seed 1, 55% rock, generation ${iterations}: ${(openShare(grid) * 100).toFixed(1)}% open`);
  const x0 = panel * (panelW + GAP);
  for (let y = 0; y < outH; y++) {
    for (let x = 0; x < panelW; x++) {
      const cell = grid[Math.floor(y / SCALE) * WIDTH + Math.floor(x / SCALE)];
      rgb.set(cell ? ROCK : OPEN, (y * outW + x0 + x) * 3);
    }
  }
});

writeFileSync(join(bundle, 'assets/caves-fallback.png'), png(outW, outH, rgb));
console.log(`staged ${manifest.id}@${manifest.version} in ${bundle}`);

if (publish !== -1) {
  const site = resolve(process.env.CHRISTIANJMILLS_SITE ?? join(here, '../christianjmills'));
  const tool = join(site, 'tools/publish-deliverable.ts');
  if (!existsSync(tool)) throw new Error(`no site checkout at ${site}: set CHRISTIANJMILLS_SITE to a checkout of cj-mills/christianjmills`);
  execFileSync('node', [tool, bundle, ...publishArgs], { stdio: 'inherit', cwd: site });
}
