// The deliverable's build: stage the bundle in dist/bundle/ and optionally hand it to the site's
// publish command.
//
//   node build.mjs                     stage the bundle only
//   node build.mjs --publish <args>    stage, then run the site's publish command with <args>
//   node build.mjs --draft             stage a page that isn't traced yet, to look at it on the
//                                      site's preview server: its citations are left out, and it
//                                      can't be published
//
// What staging does, each step refusing rather than guessing:
//   1. writes the page again from page/content/ and lab/data/ (page/gen.mjs) and refuses a
//      committed page that differs: every table, step and recipe link is regenerated;
//   2. resolves the page's passage-id citations into the site's locators (`slib trace`'s file);
//   3. checks every {% own-run %}: each value it names is regenerated from the data or the core
//      and must be what the page states;
//   4. fills the manifest's `checked` from the checking note (`slib note`'s file);
//   5. builds the core as WebAssembly (lab/measure-size.mjs) and folds its glue into the module;
//   6. draws every step figure from the core's own frames, for both themes;
//   7. adds the measurement data and the recipe schema, and names the commit it was built from.
//
// It needs the repo's pinned Rust toolchain (rust-toolchain.toml, mise.toml) and Node; it has no
// other dependencies. The publish command belongs to the site, which owns the deliverable format.
// It is run from a checkout of cj-mills/christianjmills: $CHRISTIANJMILLS_SITE, or
// ../christianjmills beside this repo.
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { gzipSync } from 'node:zlib';
import { applyCitations } from './apply-citations.mjs';
import { PAGE, generate } from './page/gen.mjs';
import { NEW_TINT, build, diagram, figure, siteTokens, tokens, value } from './page/lib.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const source = join(here, 'deliverable');
const bundle = join(here, 'dist/bundle');
const args = process.argv.slice(2);
const publish = args.indexOf('--publish');
const publishArgs = publish === -1 ? [] : args.slice(publish + 1);
const draft = args.includes('--draft');
if (draft && publish !== -1) throw new Error('a draft is not published: trace the page, then build without --draft');
const site = resolve(process.env.CHRISTIANJMILLS_SITE ?? join(here, '../christianjmills'));

const git = (...a) => execFileSync('git', a, { cwd: here, encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'] }).trim();
const commit = git('rev-parse', '--verify', 'HEAD');
const clean = git('status', '--porcelain') === '';

// A bundle published to the real bucket names the commit it was built from, so that commit must
// be clean and on GitHub. A local stand-in publish may come from an unpushed or dirty tree.
if (publishArgs.includes('--r2')) {
  if (!clean) throw new Error('commit your changes first: a published bundle names the commit it was built from');
  if (git('branch', '--remotes', '--contains', commit) === '') throw new Error(`push ${commit} first: the bundle names it`);
}

// 1. The page is what the generator writes.
const { page: fresh, figures, diagrams, blocks, data } = generate();
const page = readFileSync(PAGE);
if (page.toString('utf8') !== fresh) throw new Error('deliverable/content/index.mdoc is not what page/gen.mjs writes: run `node page/gen.mjs`, then trace it again');

rmSync(bundle, { recursive: true, force: true });
cpSync(source, bundle, { recursive: true });
const manifestPath = join(bundle, 'manifest.json');
const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
const entry = join(source, manifest.content.entry);
const inBundle = (suffix) => join(bundle, manifest.content.entry.replace(/\.mdoc$/, suffix));

// 2. The page cites passages by id ({% cite id="…" /%}). source-library's `slib trace` resolved
// them into the site's locators in the committed citations file; applying it is plain string
// splicing, refused if the page changed since it was traced. The file stays out of the bundle.
if (draft) writeFileSync(inBundle('.mdoc'), page.toString('utf8').replace(/^---\n[\s\S]*?\n---\n/, '').replace(/ ?\{%\s*(cite|own|own-run)\b[^%]*%\}/g, ''));
else writeFileSync(inBundle('.mdoc'), applyCitations(page, JSON.parse(readFileSync(entry.replace(/\.mdoc$/, '.citations.json'), 'utf8'))));
rmSync(inBundle('.citations.json'), { force: true });

// 3. Every value the page marks {% own-run values="…" /%} is regenerated here. A table or a list
// (table.…, list.…) must be on the page as the generator writes it; any other value must be in
// the sentence that names it.
const text = page.toString('utf8');
let ownRuns = 0, values = 0;
const problems = [];
for (const m of text.matchAll(/\{%\s*own-run\b([^%]*)%\}/g)) {
  ownRuns++;
  const names = /values="([^"]*)"/.exec(m[1])?.[1].split(/\s+/).filter(Boolean) ?? [];
  const line = text.slice(0, m.index).split('\n').length;
  if (!names.length) problems.push(`line ${line}: an own-run names no values`);
  const sentence = text.slice(Math.max(text.lastIndexOf('\n', m.index), text.lastIndexOf('%}', m.index - 1)) + 1, m.index);
  for (const name of names) {
    values++;
    if (/^(table|list)\./.test(name)) {
      if (!blocks.has(name)) problems.push(`line ${line}: no table or list named ${name}`);
      else if (!text.includes(blocks.get(name))) problems.push(`line ${line}: ${name} is not on the page as generated`);
    } else {
      const stated = value(name);
      if (!new RegExp(`(^|[^0-9.,])${stated.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}($|[^0-9]|[.,](?![0-9]))`).test(sentence)) problems.push(`line ${line}: ${name} is ${stated}, which its sentence doesn't state`);
    }
  }
}
if (problems.length) throw new Error(`the page's own numbers don't match the data:\n  ${problems.join('\n  ')}`);

// 4. The data for the page's "how this was checked" note, written by `slib note` from the private
// evidence: it must describe this exact page, and a published bundle may carry no open flags.
if (!draft) {
const checkingFile = entry.replace(/\.mdoc$/, '.checking.json');
const checking = JSON.parse(readFileSync(checkingFile, 'utf8'));
rmSync(inBundle('.checking.json'), { force: true });
if (checking.format !== 'slib-checking@1') throw new Error(`${checkingFile}: not a slib-checking@1 file`);
if (checking.source_sha256 !== createHash('sha256').update(page).digest('hex')) throw new Error(`${checkingFile} describes another version of the page; run \`slib note\` on it`);
if (checking.open.length && publishArgs.includes('--r2')) throw new Error(`open flags: ${checking.open.map((o) => `${o.claim ? `claim ${o.claim}` : 'page'} ${o.state}`).join(', ')}`);
if (ownRuns !== checking.claims.own_run) throw new Error(`the page has ${ownRuns} own-run claims, the checking note ${checking.claims.own_run}`);
const implementations = { 'claude-tier': 'Claude tier' };
// Before the verifier has run there is no note to print: the site's manifest wants a date for it.
if (checking.last_verified) manifest.checked = {
  claims: checking.claims.cited,
  supported: checking.claims.verified,
  measured: ownRuns,
  verifiers: checking.verifiers.map((v) => ({ name: `${v.contract}, ${implementations[v.implementation] ?? v.implementation}`, version: v.version, ...(v.model ? { model: v.model } : {}) })),
  lastVerified: checking.last_verified,
  openFlags: checking.open.length,
};
else console.warn('no claim is verified yet: this bundle has no checking note, and can only be previewed');
console.log(`checked: ${checking.claims.verified} of ${checking.claims.cited} cited claims supported (last ${checking.last_verified}); ${ownRuns} own-run claims, ${values} values regenerated; ${checking.open.length} open flags`);
} else rmSync(inBundle('.checking.json'), { force: true });

// 5. The core as WebAssembly, built and measured by the lab's own script. Its wasm-bindgen glue is
// folded into the module, inside a function of its own so no name of the glue's meets one of the
// module's: one file less to fetch before the lab can mount.
execFileSync('node', [join(here, 'lab/measure-size.mjs')], { stdio: ['ignore', 'ignore', 'inherit'] });
const web = join(here, 'lab/target/web');
const modulePath = join(bundle, 'assets/lab/lab.js');
const moduleSource = readFileSync(modulePath, 'utf8');
const importLine = /^import init, \{ ([\w, ]+) \} from '\.\/map_lab_wasm\.js';\n/m.exec(moduleSource);
const glue = readFileSync(join(web, 'map_lab_wasm.js'), 'utf8');
const ending = "export { initSync, __wbg_init as default };\n";
if (!importLine || !glue.endsWith(ending) || /^import /m.test(glue)) throw new Error('the module or the glue is not shaped as this build folds them');
const folded = glue.slice(0, -ending.length).replace(/^export (class|function) /gm, '$1 ');
if (/^export /m.test(folded)) throw new Error('the glue has an export this build does not know how to fold');
// The module's names for things and the demos it loads by id are the page's own (page/gen.mjs).
const dataLine = /^const DATA = .*;$/m;
if (!dataLine.test(moduleSource)) throw new Error('the module has no DATA line for this build to fill');
writeFileSync(modulePath, moduleSource.replace(importLine[0], `const { init, ${importLine[1]} } = (() => {\n${folded}return { init: __wbg_init, ${importLine[1]} };\n})();\n`).replace(dataLine, () => `const DATA = ${JSON.stringify(data)};`));
copyFileSync(join(web, 'map_lab_wasm_bg.wasm'), join(bundle, 'assets/lab/map_lab_wasm_bg.wasm'));
const tint = /export const NEW_TINT = ([\d.]+);/.exec(moduleSource)?.[1];
if (Number(tint) !== NEW_TINT) throw new Error(`the module tints new tiles ${tint}, the figures ${NEW_TINT}: page/lib.mjs and the module must agree`);
const gz = (p) => gzipSync(readFileSync(p)).byteLength;
console.log(`module ${gz(modulePath)} B gzipped with the glue, .wasm ${gz(join(bundle, 'assets/lab/map_lab_wasm_bg.wasm'))} B`);

// 6. The step figures, in the site's figure pair for each theme. The copy of the site's colors in
// page/tokens.json must be the site's, when a checkout of it is here to compare.
const theirs = siteTokens(site);
if (theirs) {
  for (const [group, names] of Object.entries({ color: tokens.color, figure: tokens.figure }))
    for (const name of Object.keys(names))
      if (JSON.stringify(theirs[group][name].$value) !== JSON.stringify(tokens[group][name])) throw new Error(`page/tokens.json: ${group}.${name} is not the site's (${site}/src/styles/tokens.json)`);
} else console.warn(`no site checkout at ${site}: the figures' colors are not checked against the site's tokens`);
mkdirSync(join(bundle, 'assets/steps'), { recursive: true });
manifest.provenance = [];
for (const [name, { recipe, seed, frame }] of figures) {
  const b = build(recipe, seed);
  for (const theme of ['light', 'dark']) {
    writeFileSync(join(bundle, `assets/steps/${name}-${theme}.png`), figure(b, frame, theme));
    manifest.provenance.push({ asset: `assets/steps/${name}-${theme}.png`, generator: `build.mjs, from the map lab core in lab/: step ${frame + 1} of the recipe ${JSON.stringify(recipe.pipeline)} on seed ${seed}, drawn in the site’s ${theme} figure colors` });
  }
}
mkdirSync(join(bundle, 'assets/rules'), { recursive: true });
for (const [name, { rows, key }] of diagrams) {
  for (const theme of ['light', 'dark']) {
    writeFileSync(join(bundle, `assets/rules/${name}-${theme}.png`), diagram(rows, theme, key));
    manifest.provenance.push({ asset: `assets/rules/${name}-${theme}.png`, generator: `build.mjs, from the diagram’s rows in page/content/, drawn in the site’s ${theme} figure colors` });
  }
}
console.log(`drew ${figures.size} step figures and ${diagrams.size} rule diagrams, each for both themes`);

// 7. The measurement data and the recipe schema, for agents and anyone who wants the numbers.
for (const [from, to] of [['lab/data/traits.json', 'traits.json'], ['lab/data/faithfulness.json', 'faithfulness.json'], ['lab/schema/recipe.schema.json', 'recipe.schema.json']]) copyFileSync(join(here, from), join(bundle, 'assets/lab', to));

if (clean) manifest.code.commit = commit;
else console.warn('working tree has changes: the bundle names no commit');
writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + '\n');
console.log(`staged ${manifest.id}@${manifest.version} in ${bundle}`);

if (publish !== -1) {
  const tool = join(site, 'tools/publish-deliverable.ts');
  if (!existsSync(tool)) throw new Error(`no site checkout at ${site}: set CHRISTIANJMILLS_SITE to a checkout of cj-mills/christianjmills`);
  execFileSync('node', [tool, bundle, ...publishArgs], { stdio: 'inherit', cwd: site });
}
