// v1/jsx-no-undef on dotted tags: the root identifier is judged on its own —
// a resolved namespace import passes, an unbound root reports, and the
// member's existence is TypeScript's question, not this rule's. The handler
// value exercises the static-string folder: a conditional that merely starts
// and ends with a quote character is not a string literal.
import * as Widgets from "./widgets";

export function Dotted(props: { toggle: () => void; name: string }) {
  return (
    <div>
      <Widgets.Card />
      <Missing.Thing />
      <button onClick={'a' === props.name ? props.toggle : 'b'} />
    </div>
  );
}
