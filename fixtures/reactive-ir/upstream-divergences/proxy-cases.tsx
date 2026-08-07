// v1/no-proxy-apis judges every mergeProps argument (functions and
// non-props identifiers Proxy, object literals and props pass) and every
// member expression or call anywhere under a JSX spread; and
// v1/jsx-no-duplicate-props reads only a literal spread object's own keys.
import { mergeProps } from "solid-js";

const source = { a: 1 };

export function Proxies(props: { name: string }) {
  const merged = mergeProps({}, () => ({ b: 2 }));
  const fine = mergeProps({ a: 1 }, props);
  void merged;
  void fine;
  return (
    <>
      <div {...{ a: source.a }} />
      <div b="x" {...{ a: { b: 1 } }} />
      <div a="1" {...{ a: 2 }} />
    </>
  );
}
