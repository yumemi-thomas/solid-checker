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
      {/* WHATWG scope boundaries: the inner list stops the li walk, the
          button takes p out of button scope, and the td terminates the
          default scope around the inner button. */}
      <ul><li><ul><li /></ul></li></ul>
      <dl><dd><dl><dt /></dl></dd></dl>
      <p><button><div /></button></p>
      <button><table><tbody><tr><td><button /></td></tr></tbody></table></button>
    </main>
  );
}
