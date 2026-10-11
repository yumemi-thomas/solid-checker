import { makeSize } from "reactive-package";
export function TypedKeysAlias() {
  const location = makeSize();
  const keys: typeof Object.keys = () => [];
  const names = keys(location);
  return <div>{String(names)}</div>;
}
