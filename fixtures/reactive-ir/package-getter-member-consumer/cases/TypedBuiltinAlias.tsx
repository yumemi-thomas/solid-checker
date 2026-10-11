import { makeSize } from "reactive-package";
export function TypedBuiltinAlias() {
  const location = makeSize();
  const fake: Pick<ObjectConstructor, "assign"> = { assign: Object.assign };
  const copy = fake.assign({}, location);
  return <div>{copy.pathname}</div>;
}
