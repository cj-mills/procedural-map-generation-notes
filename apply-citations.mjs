// Apply a source-library citations file (slib-citations@1) to a deliverable's Markdoc source:
// replace each run of {% cite %} / {% own %} tags with the site's {% ref %} tags, and drop the
// frontmatter. Dependency-free, for a content project's bundle build; the reference
// implementation is `apply()` in source_library/resolve.py, and the tests run both.
//
//   import { applyCitations } from './apply-citations.mjs';
//   const out = applyCitations(sourceBytes, JSON.parse(citationsJson));  // a string, or throws
//
// MIT licence. Copied into projects verbatim; source-library's tests check it.
import { createHash } from 'node:crypto';

export function applyCitations(bytes, lock) {
  if (lock.format !== 'slib-citations@1') throw new Error('not a slib-citations@1 file');
  const digest = createHash('sha256').update(bytes).digest('hex');
  if (digest !== lock.source_sha256)
    throw new Error(`${lock.source} changed since its citations were resolved; run \`slib trace\` on it`);
  if (lock.unresolved?.length) throw new Error(`unresolved passages: ${lock.unresolved.join(', ')}`);
  const text = Buffer.from(bytes).toString('utf8');
  const lineOf = (i) => text.slice(0, i).split('\n').length - 1;
  let out = '';
  let pos = 0;
  for (const s of lock.splices) {
    let i = text.indexOf(s.match, pos);
    while (i >= 0 && lineOf(i) !== s.line) i = text.indexOf(s.match, i + 1);
    if (i < 0) throw new Error(`line ${s.line + 1}: ${JSON.stringify(s.match.slice(0, 60))} not found`);
    out += text.slice(pos, i) + s.replace;
    pos = i + s.match.length;
  }
  return out + text.slice(pos);
}

// As a command: node apply-citations.mjs <source.mdoc> <citations.json> <out.mdoc>
if (import.meta.url === `file://${process.argv[1]}`) {
  const { readFileSync, writeFileSync } = await import('node:fs');
  const [src, lockPath, outPath] = process.argv.slice(2);
  writeFileSync(outPath, applyCitations(readFileSync(src), JSON.parse(readFileSync(lockPath, 'utf8'))));
}
