// A local function spelled `flush`: not the primitive, so no drain.
function flush() {
  return 1;
}

export function Shadowed() {
  flush();
  return <div />;
}
