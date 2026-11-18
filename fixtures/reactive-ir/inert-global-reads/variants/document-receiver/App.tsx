import "@ws/document-receiver";

// Calls of globals the libraries declare, in a component body. A package that loads Solid and may write any global makes each a site (ADR 0974).
export function Globals() {
  btoa("x");
  Math.max(1, 2);
  JSON.parse("1");
  return null;
}
