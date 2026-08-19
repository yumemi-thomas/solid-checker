// v1/no-proxy-apis proves inline/resolved functions use Proxy, proves exact
// plain object literals do not, and keeps every other mergeProps source
// uncertifiable without trusting identifier spellings such as `props`.
// It also judges every member expression or call under a JSX spread; and
// v1/jsx-no-duplicate-props reads only a literal spread object's own keys.
import { mergeProps } from "solid-js";

const source = { a: 1 };

export function Proxies(props: { name: string }) {
  const merged = mergeProps({}, () => ({ b: 2 }));
  const uncertain = mergeProps({ a: 1 }, props);
  void merged;
  void uncertain;
  return (
    <>
      <div {...{ a: source.a }} />
      <div b="x" {...{ a: { b: 1 } }} />
      <div a="1" {...{ a: 2 }} />
    </>
  );
}
