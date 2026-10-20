import { Provider, useCtx } from "dep-ctx";
import { StaticWrap } from "dep-wrap";

export function A() {
  const api = useCtx()!;
  const o = api.orientation();
  return <p>{o}</p>;
}
export function Root() {
  return <Provider><StaticWrap><A /></StaticWrap></Provider>;
}
