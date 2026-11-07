// node show-census.mjs <wt> <code> <context> [substring]: print the mx-census programs of a cell
const [wt, code, context, want] = process.argv.slice(2);
const m = await import(`${wt}/scripts/fuzz/matrix.mjs`);
const cell = { code, context };
m.censusShapes(code).forEach((shape, index) => {
  const id = m.shapeId(cell, shape);
  if (want && !id.includes(want)) return;
  const { built } = m.generateShape({ seed: "1" }, cell, shape, index);
  console.log(`===== ${id} sites=${JSON.stringify(built.sites?.map?.((s) => s.line))}`);
  console.log(built.text);
});
