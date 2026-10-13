import { evens } from "./helpers";

export function App(props: { items: number[]; keep: (fn: (list: number[]) => number[]) => void }) {
  // Clean: `forward` is entered only through calls, each passing an array
  // built at the call.
  const forward = (list: number[]) => evens(list);
  // Clean: followed one function further back.
  const twice = (list: number[]) => forward(list);
  // Clean: a default array, a call that omits the argument, and a literal.
  const withDefault = (list: number[] = []) => evens(list);
  // `SC9012`: one call site passes a caller-supplied value.
  const mixed = (list: number[]) => evens(list);
  // `SC9012`: the parameter is written in the body.
  const rewritten = (list: number[]) => {
    list = list.slice();
    return evens(list);
  };
  // `SC9012`: the function is handed out as a value, so its callers are not
  // all visible.
  const escaping = (list: number[]) => evens(list);
  props.keep(escaping);
  const total =
    forward([1, 2]).length +
    twice([3, 4]).length +
    withDefault().length +
    withDefault([5]).length +
    mixed([6]).length +
    mixed(props.items).length +
    rewritten([7]).length +
    escaping([8]).length;
  return <div>{total}</div>;
}
