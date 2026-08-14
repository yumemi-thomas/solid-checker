// v1/jsx-no-undef keeps resolved namespace imports clean. An absent semantic
// fact for an unbound root is uncertifiable rather than proof of undefined;
// the member's existence is TypeScript's question, not this rule's. The
// handler value exercises the static-string folder: a conditional that merely
// starts and ends with a quote character is not a string literal.
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
