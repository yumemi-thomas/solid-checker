declare const Child: (props: { children?: unknown }) => unknown;

export function InvalidNesting() {
  return (
    <main>
      <p><p /></p>
      <table><tr><td /></tr></table>
      <tr><div /></tr>
      <button><span><button /></span></button>
      <a href="#outer"><span><a href="#inner" /></span></a>
    </main>
  );
}

export function ValidNesting() {
  return (
    <main>
      <table><tbody><tr><td /></tr></tbody></table>
      <ul><li /></ul>
      <p><Child><div /></Child></p>
      <svg><a><a /></a></svg>
      <svg><foreignObject><p><span /></p></foreignObject></svg>
    </main>
  );
}
