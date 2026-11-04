# Notes on Procedural Map Generation Techniques

The source of [christianjmills.com/posts/procedural-map-generation-techniques-notes/](https://christianjmills.com/posts/procedural-map-generation-techniques-notes/):
notes on Herbert Wolverson's talk
[Procedural Map Generation Techniques](https://www.youtube.com/watch?v=TlLIOgWYVpI)
(Roguelike Celebration 2020), with an interactive demo for each technique. Every claim about
the talk is cited to its slides by page or to its code
([thebracket/roguelike-celebration-2020](https://github.com/thebracket/roguelike-celebration-2020))
by line.

That URL still serves the old post until the site relaunches. So far the page has one section, cellular-automata caves.

## Layout

```
deliverable/
  manifest.json        the deliverable's manifest (format 0.1)
  content/index.mdoc   the page, in the site's Markdoc dialect
  assets/caves.js      the cave interactive; build.mjs also runs it to draw the fallback image
build.mjs              stages the bundle in dist/bundle/ and publishes it
```

The page is published to the site as a deliverable bundle. The site repo,
[cj-mills/christianjmills](https://github.com/cj-mills/christianjmills), owns the format: its
`spec/README.md` describes the manifest, the dialect and the bundle.

## Build and publish

Node 26.13.0. There are no dependencies.

```sh
npm run build
```

stages the bundle in `dist/bundle/`: the deliverable's files, the fallback image rendered from
`caves.js`, and the manifest with `code.commit` set to `HEAD` when the working tree is clean.

Publishing runs the site's publish command, which verifies the bundle against the site's spec,
uploads it and pins it in the site's lockfile. It needs a checkout of the site repo with its
dependencies installed (`npm ci`), at `../christianjmills` beside this repo or wherever
`CHRISTIANJMILLS_SITE` points:

```sh
npm run publish:r2      # to the R2 bucket, with credentials from the site's tools/cloudflare.sh
DELIVERABLES_PUBLIC_URL=file:///tmp/bucket/ npm run publish:local   # to a local folder laid out like the bucket
```

`publish:r2` refuses a dirty tree or a commit that isn't pushed, because the bundle names the
commit it was built from. The lockfile change lands in the site checkout, to be committed
there.

To preview without publishing, point the site's `deliverables.local.json` at `dist/bundle/`
and run the site's `npm run dev`.

## License

The code is MIT (`LICENSE`). The prose is CC BY-NC-ND 4.0 (`LICENSE-PROSE`).
