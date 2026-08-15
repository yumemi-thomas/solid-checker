// Public findings-snapshot coverage for the two shared JSX-correctness
// rules: SC8019 no-implicit-draggable and SC8020 valid-jsx-nesting.
//
// The draggable expectations are pinned to @solidjs/web@2.0.0-rc.0 (probed):
// shorthand and a literal `true` both render the bare attribute on the
// client (setAttribute("draggable", "")) and the server (`<div draggable>`),
// selecting the enumerated attribute's invalid-value default `auto` — never
// draggable="true". Only the string forms select a real state.
//
// The nesting expectations follow the WHATWG in-body rules *with their scope
// boundaries*: an implied-end-tag walk stops at special elements (nested
// lists are preserved verbatim), a p closes only when in button scope, and
// button/a use the default scope list.

const canDrag = () => true;

export function Draggable() {
  return (
    <section>
      <img draggable />                                {/* SC8019: presence-only -> auto */}
      <img draggable={true} />                         {/* SC8019: also presence-only -> auto */}
      <img draggable="true" />                         {/* negative: real "true" state */}
      <img draggable="false" />                        {/* negative: real "false" state */}
      <img draggable={canDrag() ? "true" : "false"} /> {/* negative: dynamic string */}
    </section>
  );
}

export function ValidNesting() {
  return (
    <main>
      {/* The inner <ul> is special and stops the li implied-end-tag walk. */}
      <ul><li>outer<ul><li>inner</li></ul></li></ul>
      {/* Same for definition lists: <dl> stops the dd/dt walk. */}
      <dl><dt>term</dt><dd>outer<dl><dd>inner</dd></dl></dd></dl>
      {/* The <button> terminates button scope, so the div does not close the p. */}
      <p><button><div /></button></p>
      {/* The <td> terminates the default scope, preserving the outer button. */}
      <button><table><tbody><tr><td><button /></td></tr></tbody></table></button>
      {/* Complete table structure. */}
      <table><tbody><tr><td /></tr></tbody></table>
    </main>
  );
}

export function InvalidNesting() {
  return (
    <main>
      {/* SC8020: li in li with no list boundary between them. */}
      <ul><li>first<li>second</li></li></ul>
      {/* SC8020: button in button (default scope, span is no boundary). */}
      <button><span><button /></span></button>
      {/* SC8020: a in a. */}
      <a href="#outer"><span><a href="#inner" /></span></a>
      {/* SC8020: the div closes the p (p is in button scope here). */}
      <p><div /></p>
      {/* SC8020: the parser moves the tr into an inserted tbody. */}
      <table><tr><td /></tr></table>
    </main>
  );
}
