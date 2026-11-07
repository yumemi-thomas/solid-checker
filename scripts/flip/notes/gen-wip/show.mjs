// usage: node show.mjs <worktree> <code> <context> <id-substring>... ; prints generated grid programs (recall lean shapes)
const [wt, code, context, ...want] = process.argv.slice(2);
const m = await import(`${wt}/scripts/fuzz/matrix.mjs`);

const shapes = m.recallShapes(code, context, { lean: true });
const cell = { code, context };
shapes.forEach((shape, index) => {
  const id = m.shapeId(cell, shape);
  if (!want.some((w) => id === w || (w.endsWith("*") && id.startsWith(w.slice(0, -1))))) return;
  const { built } = m.generateShape({ seed: "1" }, cell, shape, index);
  console.log(`===== ${id} sites=${JSON.stringify(built.sites?.map?.((s) => s.line) ?? built.sites)}`);
  console.log(built.text ?? JSON.stringify(Object.keys(built)));
});
