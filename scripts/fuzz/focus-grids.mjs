// The fuzzer's grids, by name: focus.mjs runs them, cover.mjs samples them.
const MOUNTS = ["--mounts", "root,child,show,for"];
const SOURCES = ["--sources", "props,context,module,async"];
/** The grids of the full run (the same arguments), by name. */
export const GRIDS = {
  "mx-base": ["matrix.mjs", ...MOUNTS],
  "mx-src": ["matrix.mjs", ...SOURCES],
  "mx-combo": ["matrix.mjs", "--combine", ...MOUNTS],
  "lg-base": ["legal.mjs", ...MOUNTS],
  "lg-src": ["legal.mjs", ...SOURCES],
  "lg-combo": ["legal.mjs", "--combine", ...MOUNTS],
  // The recall axes (matrix.mjs recallAxes): asked for by name (`--grids mx-recall,lg-recall`), never by default, and lean unless `--recall-full` (matrix.mjs LEAN_DEMOTED); their false cleans
  // are the measured known misses, not regressions.
  "mx-recall": ["matrix.mjs", "--recall"],
  "lg-recall": ["legal.mjs", "--recall"],
  // The pair grids (matrix.mjs pairShapes, misuse.mjs PAIR_SETS): asked for by name too, `--sample-big` when iterating.
  "mx-pairs": ["matrix.mjs", "--pairs", "all"],
  "lg-pairs": ["legal.mjs", "--pairs", "all"],
  // The census (matrix.mjs censusCells): every (code, context) no cell lists, plain shapes, a default grid (about 3,200 programs, a minute or two).
  "mx-census": ["matrix.mjs", "--census"],
};
