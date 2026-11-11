// The open lab's rows, and the room its page reserves for it.
//
// Open, the lab's height is fixed by its layout: the map follows the width, and every other row
// has a set height in rem (the lab's type is fluid with the window, so a row is sized for its
// content at the type's largest). The module's CSS is written from ROWS, and RESERVE is their
// sum: the length page/gen.mjs writes on the head lab's tag, which the site holds the figure's
// live area to from the first paint (the site's spec, Interactives, "A reserve"). One owner, so
// the tag and the layout can't drift apart; the site's build checks the two against each other
// at 16 window sizes.
//
// The build folds this file into the module (build.mjs), so the page loads one file.

/** Row heights and gaps in rem. A layout's `at` is the root's width, in rem, where it starts. */
export const ROWS = {
  gap: 0.75, // between the lab's parts
  fitGap: 0.5, // between the rows of the drawing and its controls
  legend: 1.75, // the legend's line, with the space above it
  strip: 3.75, // the pipeline: a chip's two lines, and room for a scrollbar under them
  control: 2, // one of a stage's controls
  controlGap: 0.25,
  action: 2.25, // a row of the actions: the walk, measuring, the demo to start from
  actionGap: 0.5,
  trait: 3.125, // a trait's label, its value and its scale
  traitGap: 0.5,
  recipe: 2.25, // the recipe's summary line
  // One column. The step buttons in a row; a stage's title, on up to two lines, beside its
  // buttons, then its controls in three rows (five go two to a row); the actions in three rows;
  // the traits three to a row, over two lines on what fails.
  narrow: { transport: 2.25, head: 2.25, controls: 3, actions: 3, traits: 3, fails: 2.625 },
  // One column with room: as above, with the actions in two rows and the traits five to a row.
  mid: { at: 30, transport: 2.25, head: 2.25, controls: 3, actions: 2, traits: 2, fails: 2.625 },
  // Two columns. Beside the map: the step buttons in two rows, a stage's title over its buttons,
  // and five controls in five rows. The step's text beside the actions' three rows. The traits
  // five to a row, over one line on what fails.
  wide: { at: 44, transport: 4.75, head: 4, controls: 5, actions: 3, traits: 2, fails: 1.5 },
  map: 0.66, // two columns: the map's share of the width
};

const r = (n) => `${+n.toFixed(4)}rem`;
/** A stage's slot in a layout: its head, then room for the most controls a stage has. */
export const stageSlot = (layout) => layout.head + layout.controls * (ROWS.control + ROWS.controlGap);
/** The actions' slot in a layout. */
export const actionsSlot = (layout) => layout.actions * (ROWS.action + ROWS.actionGap) - ROWS.actionGap;
/** The last part's slot in a layout: every trait the core reports, what fails, and the recipe's line. */
export const endSlot = (layout) => layout.traits * (ROWS.trait + ROWS.traitGap) + layout.fails + ROWS.traitGap + ROWS.recipe;

// The map is 80 by 50 tiles on a canvas of 640 by 400 px that is never drawn larger, inside a
// 1 px border: its height is 62.5% of its width, less the border's share.
const MAP_ASPECT = 0.625, MAP_MOST = '402px', BORDER = '0.75px';
/** The map's height in one column, before the window has its say. */
export const MAP_NARROW = `min(${MAP_ASPECT * 100}cqi + ${BORDER}, ${MAP_MOST})`;
/** The map's height in two columns. */
export const MAP_WIDE = `min(${+(MAP_ASPECT * ROWS.map * 100).toFixed(4)}cqi + ${BORDER}, ${MAP_MOST})`;

/** The slot for the step's text. In one column it follows the width: the longest step's text is
 *  six lines under its label on the narrowest phone and three where the column is widest. In two
 *  columns the text sits beside the actions, and their three rows are taller than it. */
export const SAYS_NARROW = 'max(6.5rem, 12.25rem - 16cqi)';
export const SAYS_WIDE = r(actionsSlot(ROWS.wide));

/** What the drawing's part holds beside the map, in one column: the legend, the step buttons,
 *  the strip and the stage's slot, with `extra` rem more for a card's row of demos. */
export const fitRestNarrow = (extra = 0) => ROWS.legend + ROWS.narrow.transport + ROWS.strip + stageSlot(ROWS.narrow) + 3 * ROWS.fitGap + extra;
/** The map's slot in one column. The drawing and its controls stay within the block size the
 *  site allows by drawing the map smaller, where that works: a map at under half its size shows
 *  too little to be worth the fit, so where the window is too short for the two together at any
 *  size worth drawing (a phone on its side) the map keeps the column's width. `clamp()` picks:
 *  its middle term is far below zero when half the map fits, and far above when it doesn't. */
export const mapSlotNarrow = (extra = 0) => {
  const rest = r(fitRestNarrow(extra)), limit = 'var(--interactive-max-block-size)';
  return `clamp(min(${MAP_NARROW}, ${limit} - ${rest}), (${MAP_NARROW} * 0.5 + ${rest} - ${limit}) * 9999, ${MAP_NARROW})`;
};
/** Two columns: the map's row is as tall as the map and its legend, or as the step buttons and
 *  the stage's slot beside them, whichever is taller. */
export const WIDE_SIDE = ROWS.wide.transport + ROWS.fitGap + stageSlot(ROWS.wide);

/** `clamp()` picks a layout as it picks the map's size above: `from` while the root is `at` rem
 *  wide or wider, `below` while it is narrower. `below` is the taller of the two at every width
 *  where it is the one picked, which is all `clamp()` needs. */
const pick = (from, at, below) => `clamp(${from}, (${at}rem - 100cqi) * 9999, ${below})`;
// One column: the map's slot, the step's text's, and what the layout's other rows come to.
const rest = (layout) => r(fitRestNarrow() + actionsSlot(layout) + endSlot(layout) + 3 * ROWS.gap);
const oneColumn = `${mapSlotNarrow()} + ${SAYS_NARROW} + ${pick(rest(ROWS.mid), ROWS.mid.at, rest(ROWS.narrow))}`;
const twoColumns = `max(${MAP_WIDE} + ${r(ROWS.legend)}, ${r(WIDE_SIDE)}) + ${r(ROWS.strip + ROWS.fitGap + actionsSlot(ROWS.wide) + endSlot(ROWS.wide) + 2 * ROWS.gap)}`;

/** The open lab's height: the length the page reserves. */
export const RESERVE = pick(twoColumns, ROWS.wide.at, oneColumn);
