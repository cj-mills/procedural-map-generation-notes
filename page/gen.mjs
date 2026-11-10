// Writes the page, deliverable/content/index.mdoc, from what is written by hand in page/content/
// and what is measured in lab/data/. The page is committed, because `slib trace` and the verifier
// read it; the bundle's build runs this again and refuses a committed page that differs, so no
// number, table, step or recipe link on the page can drift from the data and the core.
//
//   node page/gen.mjs            write the page
//   node page/gen.mjs --check    fail if the committed page isn't what this writes
//
// In page/content/:
//   page.mdoc            the page's frame: its sections, with a directive where a generated part goes
//   notes.mdoc           each kind of step's text, under "## <key>"; {n} is the step's number.
//                        "## <key>#<demo>" is the text for one demo, where it differs.
//   cards/<stage>.mdoc   a technique's card: its opening line, then its seven parts under "## <part>"
//
// Directives are lines starting with @@. {{name}} is a measured value (lib.mjs, value()); a
// sentence that states one names it in its {% own-run values="…" /%}.
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  DEMOS, EXTRA_TRAITS, FAIL_NAMES, GROUPS, N, OWN, SCALE, SHORT, SUBJECT, TRAITS, build, cleanRecipe, failuresOf, faith, faithTally, flatStages, fmt, pickFrames,
  pipelineWords, recipeLink, root, short, stageTable, traits, value,
} from './lib.mjs';

const content = join(root, 'page/content');
/** What is written by hand, with its short citations written out: [[t:73-74 s:9 c03:61-66 own run:c03.regions.median]]
 *  cites the talk's transcript segments 73 and 74, its slide 9 and lines 61 to 66 of the demo c03's code, and marks
 *  the claim as partly ours and as stating a measured value. `backend` is the demos' shared map code, `g` the
 *  lines of it that say which glyph is solid, and data:<file> a file of the bundle's own. */
const SOURCES = { talk: 'wolverson2020-talk/transcript', slides: 'wolverson2020-slides/pdf', code: 'wolverson2020-code' };
const DATA = { traits: 'assets/lab/traits.json', faithfulness: 'assets/lab/faithfulness.json', schema: 'assets/lab/recipe.schema.json' };
const demoDir = (id) => Object.keys(DEMOS).find((d) => short(d) === id);
function citations(text, file) {
  // The values after run: are its list, to the next token of another kind.
  const tokens = (inner) => inner.trim().split(/\s+/).reduce((out, t) => (out.length && out.at(-1).startsWith('run:') && !t.includes(':') && t !== 'own' ? out.splice(-1, 1, `${out.at(-1)},${t}`) : out.push(t), out), []);
  return text.replace(/\[\[([^\]]+)\]\]/g, (_, inner) => tokens(inner).map((token) => {
    const [kind, what = ''] = token.split(/:(.*)/s);
    const [a, b = a] = what.split('-').map(Number);
    if (token === 'own') return '{% own /%}';
    // g: which glyph is solid in the talk's code. A claim about its walls and floor rests on these lines too.
    if (token === 'g') return `{% cite id="${SOURCES.code}/backend/src/lib.rs#L12-L16" /%}{% cite id="${SOURCES.code}/backend/src/lib.rs#L54-L65" /%}`;
    if (kind === 'run' && what) return `{% own-run values="${what.split(',').join(' ')}" /%}`;
    if (kind === 'data' && DATA[what]) return `{% ref source="this" path="${DATA[what]}" lines="1-6" /%}`;
    if (!(a >= 0 && b >= a)) throw new Error(`${file}: [[${inner}]]: ${token} is not a citation`);
    if (kind === 't') return Array.from({ length: b - a + 1 }, (_, i) => `{% cite id="${SOURCES.talk}#s${a + i}" /%}`).join('');
    if (kind === 's') return Array.from({ length: b - a + 1 }, (_, i) => `{% cite id="${SOURCES.slides}#p${a + i}" /%}`).join('');
    const path = kind === 'backend' ? 'backend/src/lib.rs' : demoDir(kind) && `${demoDir(kind)}/src/main.rs`;
    if (!path) throw new Error(`${file}: [[${inner}]]: no source ${kind}`);
    return `{% cite id="${SOURCES.code}/${path}#L${a}${b > a ? `-L${b}` : ''}" /%}`;
  }).join(''));
}
const read = (p) => citations(readFileSync(join(content, p), 'utf8'), p);
export const PAGE = join(root, 'deliverable/content/index.mdoc');
export const PARTS = ['The rule', 'Step through it', 'Controls', 'What it produces, measured', 'Where it fails', 'What it combines with', 'Checked against the talk’s code'];
const LAB = { demo: 'c20-hot-path', seed: 7 };

const attr = (s) => s.replace(/\\/g, '\\\\').replace(/"/g, '\\"');
/** A JSON value as a Markdoc attribute literal. */
function literal(v) {
  if (Array.isArray(v)) return `[${v.map(literal).join(', ')}]`;
  if (v && typeof v === 'object') return `{${Object.entries(v).map(([k, x]) => `${/^[A-Za-z_][A-Za-z0-9_]*$/.test(k) ? k : JSON.stringify(k)}: ${literal(x)}`).join(', ')}}`;
  return JSON.stringify(v);
}
const last = traits.seeds.to - 1;
/** The data files write a plain apostrophe; the page's text uses the typographic one. */
const curly = (text) => text.replaceAll("'", '’');

/** What a file's sections hold, by "## heading", with what comes before the first under "". */
function sections(text) {
  const out = new Map([['', []]]);
  let cur = '';
  for (const line of text.split('\n')) {
    const m = /^## (.+)$/.exec(line);
    if (m) out.set((cur = m[1].trim()), []);
    else out.get(cur).push(line);
  }
  return new Map([...out].map(([k, v]) => [k, v.join('\n').trim()]));
}

export function generate() {
  const table = stageTable();
  const spec = Object.fromEntries(table.stages.map((s) => [s.name, s]));
  const notes = sections(read('notes.mdoc'));
  const cards = Object.fromEntries(GROUPS.flatMap((g) => g.stages).map((stage) => {
    const text = read(`cards/${stage}.mdoc`);
    const m = /^---\n([\s\S]*?)\n---\n/.exec(text);
    if (!m) throw new Error(`cards/${stage}.mdoc: no frontmatter`);
    const meta = Object.fromEntries(m[1].split('\n').filter(Boolean).map((l) => [l.slice(0, l.indexOf(':')).trim(), l.slice(l.indexOf(':') + 1).trim()]));
    return [stage, { stage, title: meta.title, extras: JSON.parse(meta.extras ?? '[]'), parts: sections(text.slice(m[0].length)) }];
  }));
  const figures = new Map(); // name -> { recipe, seed, frame }
  const diagrams = new Map(); // name -> the rows of a rule's diagram, a character a tile (lib.mjs, diagram())
  const blocks = new Map(); // every generated table and list the page states as measured, by its value's name
  const usedKeys = new Set();
  const names = { traits: [...TRAITS, ...EXTRA_TRAITS].map((t) => ({ ...t, to: SCALE[t.key] })), fails: Object.fromEntries(Object.entries(FAIL_NAMES).map(([k, v]) => [k, v.toLowerCase()])) };

  // ---------------------------------------------------------------- steps
  const noteFor = (key, demo) => (notes.has(`${key}#${demo}`) ? `${key}#${demo}` : key);
  function step(demo, recipe, seed, b, i, before = false) {
    const f = b.frames.list[i];
    const key = noteFor(f.note, demo);
    if (!notes.get(key)) throw new Error(`notes.mdoc has no text for ${key}`);
    if (!before) usedKeys.add(f.note);
    const name = `${short(demo)}${seed === recipe.seed ? '' : `-s${seed}`}-${String(i + 1).padStart(4, '0')}`;
    figures.set(name, { recipe: cleanRecipe(recipe), seed, frame: i });
    const picture = `src="assets/steps/${name}-light.png" dark="assets/steps/${name}-dark.png" alt="${attr(`${SHORT[demo] ?? demo}, seed ${seed}: the map at step ${i + 1} of ${b.frames.list.length}.`)}"`;
    // A step of an earlier stage, shown for what this stage starts from: its own text is in its technique's card.
    if (before) return `{% step ${picture} at=${i + 1} of=${b.frames.list.length} %}\nThe map before the next step.\n{% /step %}`;
    const body = notes.get(key).replaceAll('{n}', `{% value %}${f.arg.toLocaleString('en-US')}{% /value %}`);
    return `{% step ${picture} key="${key}" at=${i + 1} of=${b.frames.list.length} %}\n${body}\n{% /step %}`;
  }
  /** A run's frames that belong to a stage, with the frame each stretch of them starts from. */
  function framesOf(b, stage) {
    const mine = b.frames.list.map((f, i) => (b.stages[f.stage] === stage ? i : -1)).filter((i) => i >= 0);
    const from = mine.filter((i) => i > 0 && !mine.includes(i - 1)).map((i) => i - 1);
    return { mine, among: [...new Set([...from, ...mine])].sort((a, b) => a - b) };
  }
  function stepsOf(card, demos, { limit = 6, mid = 0 }) {
    const out = [], seen = new Set();
    demos.forEach((d, n) => {
      const b = build(d.recipe);
      const { mine, among } = framesOf(b, card.stage);
      if (!mine.length) throw new Error(`${d.id} has no step of ${card.stage}`);
      // The first demo shows its whole run through the stage. Each demo after it shows what is new: kinds of step not yet shown, and its last.
      let pick;
      if (n === 0) pick = pickFrames(b.frames.list, among, limit, mid);
      else {
        const fresh = new Map();
        for (const i of mine) {
          const key = noteFor(b.frames.list[i].note, d.id);
          if (!seen.has(key) && !fresh.has(key)) fresh.set(key, i);
        }
        pick = [...new Set([...fresh.values(), mine.at(-1)])].sort((x, y) => x - y);
      }
      for (const i of pick) {
        const before = !mine.includes(i);
        if (!before) seen.add(noteFor(b.frames.list[i].note, d.id));
        out.push(step(d.id, d.recipe, d.recipe.seed, b, i, before));
      }
    });
    return out;
  }
  const cliLine = (d, seed) => (d.file ? `map-lab build ${d.file}${seed === undefined || seed === d.recipe.seed ? '' : ` --seed ${seed}`}` : null);

  // ---------------------------------------------------------------- a card's generated parts
  const demosOf = (stage) => traits.demos.filter((d) => SUBJECT[d.id] === stage).map((d) => ({ id: d.id, label: SHORT[d.id], file: d.recipeFile, recipe: cleanRecipe(d.recipe), d }));
  function interactive(card, opts) {
    const demos = [...demosOf(card.stage), ...card.extras.map((id) => ({ id, label: OWN[id].label, file: OWN[id].file, recipe: cleanRecipe(OWN[id].recipe) }))];
    const params = { mode: 'card', stage: card.stage, demos: demos.map((d) => d.id) };
    const what = demos.length > 1 ? `Step through ${demos.map((d) => d.label).join(', ')}` : `Step through ${demos[0].label}`;
    const lines = demos.filter((d) => d.file).map((d) => `\`${cliLine(d)}\``);
    return [
      `{% interactive id="lab" caption="${attr(`${what}: seed ${demos[0].recipe.seed}, ${demos[0].recipe.width} by ${demos[0].recipe.height} tiles. Change the seed or a control and the map is built again.`)}" params=${literal(params)} %}`,
      '{% steps %}', ...stepsOf(card, demos, opts), '{% /steps %}', '',
      `On the command line: ${lines.join(', ')}.`,
      '{% /interactive %}',
    ].join('\n');
  }
  function control(card, name, text) {
    const p = spec[card.stage].params.find((x) => x.name === name);
    if (!p) throw new Error(`${card.stage} has no parameter ${name}`);
    // The talk's value, from each of the talk's demos of this technique: every value it gives the stage.
    const given = demosOf(card.stage).map((d) => [d.id, [...new Set(findStages(d.recipe.pipeline, card.stage).map((s) => s[name] ?? p.default))]]);
    const same = given.every(([, v]) => JSON.stringify(v) === JSON.stringify(given[0][1]));
    const talk = same ? given[0][1].map((v) => `\`${v}\``).join(', ') : given.map(([id, v]) => `${v.map((x) => `\`${x}\``).join(', ')} in \`${short(id)}\``).join(', ');
    const range = p.kind === 'pick' ? p.options.map((o) => `\`${o}\``).join(', ') : `${p.min.toLocaleString('en-US')} to ${p.max.toLocaleString('en-US')}`;
    // The control's range is our stage's and the talk's value is read from our recipe of its demo: ours, apart from the text's own citations.
    return `- \`${name}\` (${range}; the talk’s: ${talk}) {% own /%}. ${text}`;
  }
  function findStages(pipeline, stage, out = []) {
    for (const s of pipeline) {
      if (s.stage === stage) out.push(s);
      for (const input of s.inputs ?? []) findStages(input, stage, out);
    }
    return out;
  }
  /** A trait's mark. The comparison table's marks say the lowest and highest too; a card's marks, which
   *  repeat them, leave those to it, and name their demo so the map on screen is marked on the right one. */
  function mark(summary, key, series) {
    if (!summary.n) return '';
    const digits = SCALE[key] <= 1 ? 3 : 1;
    const s = Object.fromEntries(['p10', 'median', 'p90', 'min', 'max'].map((k) => [k, Number(Math.min(summary[k], SCALE[key]).toFixed(digits))]));
    const ends = series ? '' : ` min=${s.min} max=${s.max} n=${summary.n}`;
    return `{% mark p10=${s.p10} median=${s.median} p90=${s.p90}${ends} unit="seeds" to=${SCALE[key]} trait="${key}"${series ? ` series="${series}"` : ''} /%}`;
  }
  function measured(card) {
    const demos = demosOf(card.stage).map((x) => x.d);
    const keys = [...TRAITS, ...EXTRA_TRAITS.filter((t) => demos.some((d) => (t.key === 'rooms' ? d.summary.rooms.n : d.summary.water.max + d.summary.grass.max + d.summary.mountain.max > 0)))];
    const rows = [`{% table label="${attr(`Measured traits: ${card.title}`)}" rowheads=true key=true %}`, '* Trait', ...demos.map((d) => `* ${SHORT[d.id]} (${short(d.id)})`), '* This map'];
    for (const { key, label, digits } of keys) {
      rows.push('---', `* [${label}](#our-definitions)`);
      for (const d of demos) {
        const s = d.summary[key];
        rows.push(s.n ? `* ${fmt(s.median, key)}, ${fmt(s.p10, key)} to ${fmt(s.p90, key)}${s.nulls ? ` (none on ${s.nulls})` : ''} ${mark(s, key, d.id)}` : '* –');
      }
      rows.push(`* {% now trait="${key}" digits=${digits} /%}`);
    }
    rows.push('{% /table %}');
    blocks.set(`table.measured.${card.stage}`, rows.join('\n'));
    return `Seeds 0 to ${last}, our own runs: the median, then the range 8 of 10 seeds fall in {% own-run values="table.measured.${card.stage}" /%}.\n\n${rows.join('\n')}`;
  }
  /** A card's failing seeds, as a table: a row a failure a demo. The criteria's wording is in "What counts as failing". */
  function fails(card) {
    const rows = [`{% table label="${attr(`Where it fails: ${card.title}`)}" rowheads=true %}`, '* Failure', '* Demo', `* Seeds failing, of ${N}`, `* The talk’s code, of ${N}`, '* Watch it fail on'];
    let any = false;
    for (const { d } of demosOf(card.stage)) {
      const un = faith[d.id].unfinished;
      for (const [f, seeds] of failuresOf(d)) {
        any = true;
        // Where the talk's loop has no end, its code fails by never finishing: the harness stops waiting after 10 seconds.
        const talk = f === 'prefab-no-place' && un ? `${un[1]}, still looking after 10 seconds` : faith[d.id].failureRates?.[f] ? value(`${short(d.id)}.talkfails.${f}`) : '–';
        const links = seeds.slice(0, 3).map((x) => `[seed ${x}](${recipeLink(d.recipe, x)})`).join(', ');
        rows.push('---', `* [${FAIL_NAMES[f]}](#what-counts-as-failing)`, `* ${SHORT[d.id]} (${short(d.id)})`, `* ${seeds.length}`, `* ${talk}`, `* ${links}${seeds.length > 3 ? `, ${seeds.length - 3} more in the data file` : ''}`);
      }
    }
    rows.push('{% /table %}');
    const none = `No seed from 0 to ${last} fails any of [our criteria](#what-counts-as-failing)`;
    blocks.set(`list.fails.${card.stage}`, any ? rows.join('\n') : none);
    return any ? `Seeds 0 to ${last} of each demo, against [our criteria](#what-counts-as-failing) {% own-run values="list.fails.${card.stage}" /%}:\n\n${rows.join('\n')}` : `${none} {% own-run values="list.fails.${card.stage}" /%}.`;
  }
  function combines(card) {
    const list = traits.demos.filter((d) => { const st = flatStages(d.recipe.pipeline); return st.includes(card.stage) && new Set(st).size > 1; })
      .map((d) => `- [${pipelineWords(d.recipe.pipeline)}](${recipeLink(d.recipe)}) \`${short(d.id)}\`: ${d.recipe.title[0].toLowerCase()}${d.recipe.title.slice(1)}`);
    return list.length ? `The talk’s demos that run it with other stages {% own /%}:\n\n${list.join('\n')}` : 'None of the talk’s demos runs it with another stage {% own /%}.';
  }
  const compared = (d) => Object.values(faith[d.id].traits).filter((v) => ['consistent', 'identical', 'differs'].includes(v.verdict));
  const differing = (d) => Object.entries(faith[d.id].traits).filter(([, v]) => v.verdict === 'differs');
  function checked(card) {
    const lines = demosOf(card.stage).map(({ d }) => {
      const c = compared(d), diff = differing(d);
      let t = `- \`${d.id}\`: ${c.length - diff.length} of ${c.length} traits consistent with the talk’s own code.`;
      for (const [k, v] of diff) t += ` **${TRAITS.find((x) => x.key === k)?.label ?? k} differs**: ours ${v.ours[0].toFixed(3)} on average, the talk’s ${v.talk[0].toFixed(3)}.`;
      return t;
    });
    blocks.set(`list.checked.${card.stage}`, lines.join('\n'));
    return `Our stage against the talk’s own code, each run on seeds 0 to ${last} and [measured the same way](#is-this-the-talks-technique) {% own-run values="list.checked.${card.stage}" /%}:\n\n${lines.join('\n')}`;
  }

  function card(stage, kind) {
    const c = cards[stage];
    const demos = demosOf(stage);
    const out = [`{% card title="${attr(c.title)}" id="${stage}" kind="${kind}" %}`, c.parts.get(''), '', `Stage \`${stage}\`. The talk’s ${demos.length > 1 ? 'demos' : 'demo'}: ${demos.map((d) => `\`${d.id}\``).join(', ')}.`, ''];
    const given = [...c.parts.keys()].filter(Boolean);
    if (JSON.stringify(given) !== JSON.stringify(PARTS)) throw new Error(`cards/${stage}.mdoc: its parts are ${given.join(' | ')}; a card has ${PARTS.join(' | ')}`);
    for (const part of PARTS) {
      out.push(`{% part title="${attr(part)}" %}`);
      const lines = c.parts.get(part).split('\n');
      for (let i = 0; i < lines.length; i++) {
        const m = /^@@(\S+)\s*(.*)$/.exec(lines[i]);
        if (!m) out.push(lines[i]);
        else if (m[1] === 'steps') out.push(interactive(c, Object.fromEntries(m[2].split(/\s+/).filter(Boolean).map((kv) => kv.split('=')).map(([k, v]) => [k, Number(v)]))));
        else if (m[1] === 'control') {
          // A control's text runs to the next directive or blank line.
          const text = [];
          while (i + 1 < lines.length && lines[i + 1].trim() && !lines[i + 1].startsWith('@@')) text.push(lines[++i].trim());
          out.push(control(c, m[2].trim(), text.join(' ')));
        } else if (m[1] === 'diagram') {
          // @@diagram <name> alt="…" caption="…", then its rows, a character a tile, to the next blank line.
          const [, name, alt, caption] = /^(\S+)\s+alt="([^"]*)"\s+caption="([^"]*)"$/.exec(m[2].trim()) ?? [];
          if (!name) throw new Error(`cards/${stage}.mdoc: @@diagram takes a name, alt="…" and caption="…"`);
          const drawn = [];
          while (i + 1 < lines.length && lines[i + 1].trim()) drawn.push(lines[++i]);
          if (!drawn.length || drawn.some((r) => r.length !== drawn[0].length)) throw new Error(`cards/${stage}.mdoc: the rows of diagram ${name} are not one width`);
          diagrams.set(name, drawn);
          out.push(`{% figure src="assets/rules/${name}-light.png" dark="assets/rules/${name}-dark.png" alt="${attr(alt)}" caption="${attr(caption)}" /%}`);
        } else if (m[1] === 'measured') out.push(measured(c));
        else if (m[1] === 'fails') out.push(fails(c));
        else if (m[1] === 'combines') out.push(combines(c));
        else if (m[1] === 'checked') out.push(checked(c));
        else throw new Error(`cards/${stage}.mdoc: no directive @@${m[1]}`);
      }
      out.push('{% /part %}', '');
    }
    const controls = [...c.parts.get('Controls').matchAll(/^@@control\s+(\S+)/gm)].map((m) => m[1]);
    const shown = spec[stage].params.filter((p) => p.kind !== 'inputs' && !(p.kind === 'pick' && p.options.length < 2)).map((p) => p.name);
    if (JSON.stringify(controls) !== JSON.stringify(shown)) throw new Error(`cards/${stage}.mdoc: its controls are ${controls.join(', ') || 'none'}; the stage has ${shown.join(', ') || 'none'}`);
    out.push('{% /card %}');
    return out.join('\n');
  }

  // ---------------------------------------------------------------- the page's generated parts
  function labPart() {
    const d = DEMOS[LAB.demo];
    const recipe = { ...cleanRecipe(d.recipe), seed: LAB.seed };
    const b = build(recipe);
    const lastOf = b.stages.map((_, k) => b.frames.list.map((f) => f.stage).lastIndexOf(k)).filter((i) => i >= 0);
    return [
      `{% interactive id="lab" caption="${attr(`The talk’s last pipeline, ${pipelineWords(recipe.pipeline)}, on seed ${LAB.seed}. Change the pipeline, a control or the seed; step through the build; then walk the map to its exit.`)}" params=${literal({ mode: 'lab', recipe })} %}`,
      '{% steps %}', ...lastOf.map((i) => step(LAB.demo, recipe, LAB.seed, b, i)), '{% /steps %}', '',
      `On the command line: \`${`map-lab build ${d.recipeFile} --seed ${LAB.seed}`}\`.`,
      '{% /interactive %}',
    ].join('\n');
  }
  function guide() {
    const D = traits.demos, s = (d, k, q = 'median') => d.summary[k][q];
    const walled = D.filter((d) => s(d, 'water', 'max') === 0 && s(d, 'floor_share') < 1); // maps of floor and wall
    const top = (list, k) => [...list].sort((a, b) => s(b, k) - s(a, k)).slice(0, 3);
    const needs = [
      ['One connected map on every seed', `one region on all ${N} seeds`, walled.filter((d) => s(d, 'regions', 'max') === 1), 'regions'],
      ['No dead ends', `no dead end on any of the ${N} seeds`, walled.filter((d) => s(d, 'dead_ends', 'max') === 0), 'dead_ends'],
      ['Many dead ends, to hide things in', 'one region on every seed, and the three highest median dead ends', top(walled.filter((d) => s(d, 'regions', 'max') === 1), 'dead_ends'), 'dead_ends'],
      ['Tight passages', 'the three highest median corridor ratios, among maps of floor and wall', top(walled, 'corridor_ratio'), 'corridor_ratio'],
      ['Open ground', 'the three highest median floor shares, among maps of floor and wall', top(walled, 'floor_share'), 'floor_share'],
      ['A long walk from start to exit', 'a path on every seed, and the three longest median paths', top(D.filter((d) => d.summary.path.nulls === 0), 'path'), 'path'],
      ['Culling before it can be played', 'more than one region on every seed', D.filter((d) => (d.failingSeeds.disconnected ?? []).length === N), 'regions'],
    ];
    const rows = ['{% table label="Which technique meets which need" rowheads=true %}', '* If the map needs', '* Our criterion', '* The talk’s demos that meet it, each with its median'];
    for (const [need, criterion, ds, key] of needs) rows.push('---', `* ${need}`, `* ${criterion[0].toUpperCase()}${criterion.slice(1)}`, `* ${ds.map((d) => `[${SHORT[d.id]}](#${SUBJECT[d.id]}) ${fmt(s(d, key), key)}`).join(', ') || 'none'}`);
    rows.push('{% /table %}');
    blocks.set('table.guide', rows.join('\n'));
    return rows.join('\n');
  }
  function comparison() {
    const rows = ['{% table label="The talk’s 22 demos compared" rowheads=true sort=true key=true %}', '* Demo', ...TRAITS.map((t) => `* [${t.label}](#our-definitions)`), `* Seeds failing, of ${N}`];
    for (const g of GROUPS) {
      rows.push('---', `* ${g.title}`);
      for (const d of traits.demos.filter((x) => g.stages.includes(SUBJECT[x.id]))) {
        rows.push('---', `* [${SHORT[d.id]}](#${SUBJECT[d.id]}) (${short(d.id)})`);
        for (const { key } of TRAITS) rows.push(`* ${fmt(d.summary[key].median ?? null, key)} ${mark(d.summary[key], key)}`.trimEnd());
        rows.push(`* ${failuresOf(d).map(([f, seeds]) => `${FAIL_NAMES[f].toLowerCase()} ${seeds.length}`).join(', ') || 'none'}`);
      }
    }
    rows.push('{% /table %}');
    blocks.set('table.comparison', rows.join('\n'));
    return rows.join('\n');
  }
  function definitions() {
    const rows = ['{% table label="The traits we measure" rowheads=true %}', '* Trait', '* Our definition'];
    for (const t of [...TRAITS, ...EXTRA_TRAITS]) rows.push('---', `* ${t.label}`, `* ${curly(traits.columns[t.key])}`);
    rows.push('{% /table %}');
    return rows.join('\n');
  }
  function failures() {
    const rows = ['{% table label="What counts as failing" rowheads=true %}', '* Failure', '* Our criterion'];
    for (const [k, v] of Object.entries(traits.failures)) rows.push('---', `* ${FAIL_NAMES[k]}`, `* ${curly(v[0].toUpperCase() + v.slice(1))}.`);
    rows.push('{% /table %}');
    return rows.join('\n');
  }
  function faithfulness() {
    const rows = ['{% table label="Faithfulness to the talk’s code" rowheads=true %}', '* Demo', '* Traits compared', '* Consistent', '* What differs'];
    for (const d of traits.demos) {
      const f = faith[d.id], c = compared(d), diff = differing(d);
      const note = diff.map(([k, v]) => `${(TRAITS.find((x) => x.key === k)?.label ?? k).toLowerCase()}: ours ${v.ours[0].toFixed(3)} on average, the talk’s ${v.talk[0].toFixed(3)}`);
      if (f.unfinished) note.push(`the talk’s loop unfinished on ${f.unfinished[1]} of ${N} seeds; ours records a failure on ${f.unfinished[0]}`);
      rows.push('---', `* [${d.id}](#${SUBJECT[d.id]}-checked-against-the-talks-code)`, `* ${c.length}`, `* ${c.length - diff.length}`, `* ${note.join('; ') || '–'}`);
    }
    rows.push('{% /table %}');
    blocks.set('table.faithfulness', rows.join('\n'));
    return rows.join('\n');
  }

  // ---------------------------------------------------------------- assemble
  const made = { lab: labPart, guide, comparison, definitions, failures, faithfulness };
  const out = ['---', 'trace: christianjmills/procedural-map-generation-techniques-notes', '---'];
  for (const line of read('page.mdoc').split('\n')) {
    const m = /^@@(\S+)\s*(.*)$/.exec(line);
    if (!m) out.push(line);
    else if (m[1] === 'group') {
      const g = GROUPS.find((x) => x.title === m[2].trim());
      if (!g) throw new Error(`page.mdoc: no group ${m[2]}`);
      out.push(g.stages.map((s) => card(s, g.kind)).join('\n\n'));
    } else if (made[m[1]]) out.push(made[m[1]]());
    else throw new Error(`page.mdoc: no directive @@${m[1]}`);
  }
  let page = out.join('\n').replace(/\n{3,}/g, '\n\n').trimEnd() + '\n';
  page = page.replace(/\{\{([^}]+)\}\}/g, (_, name) => (name.startsWith('recipe:') ? recipeLink(OWN[name.slice(7)].recipe) : value(name.trim())));
  if (/TODO/.test(page)) console.warn('the page still has TODO text');
  // Every kind of step the core can record has its text on the page, so the lab can show any pipeline's steps.
  // The core's own list of them is the `notes!` table in lab/core/src/record.rs.
  const keys = [...readFileSync(join(root, 'lab/core/src/record.rs'), 'utf8').match(/notes! \{([\s\S]*?)\n\}/)[1].matchAll(/= "([^"]+)"/g)].map((m) => m[1]);
  if (keys.length < 30) throw new Error('could not read the note keys from lab/core/src/record.rs');
  const missing = keys.filter((k) => !usedKeys.has(k));
  if (missing.length) throw new Error(`no step list shows ${missing.join(', ')}: the lab would have no text for it`);
  // What the module is given by the build: the page's names for things, and the recipes it loads by id.
  const data = {
    ...names,
    groups: GROUPS.map((g) => ({ title: g.title, stages: g.stages.map((s) => ({ name: s, title: cards[s].title })) })),
    demos: [
      ...traits.demos.map((x) => ({ id: x.id, label: SHORT[x.id], stage: SUBJECT[x.id], recipe: cleanRecipe(x.recipe) })),
      ...Object.entries(OWN).map(([id, o]) => ({ id, label: o.label, recipe: cleanRecipe(o.recipe), own: true })),
    ],
  };
  return { page, figures, diagrams, blocks, data };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const { page, figures } = generate();
  if (process.argv.includes('--check')) {
    if (!existsSync(PAGE) || readFileSync(PAGE, 'utf8') !== page) throw new Error('deliverable/content/index.mdoc is not what page/gen.mjs writes: run `node page/gen.mjs`, then trace it again');
    console.log(`the page is current: ${figures.size} step figures`);
  } else {
    writeFileSync(PAGE, page);
    console.log(`wrote ${PAGE}: ${page.length} characters, ${figures.size} step figures`);
  }
}
