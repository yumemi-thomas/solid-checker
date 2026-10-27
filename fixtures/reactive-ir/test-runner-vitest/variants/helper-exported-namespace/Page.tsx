import * as helpers from "./index";

// The namespace is handed to code that may call any export it holds from a
// component's body, so the helper's callers are not all in view.
export function Page(props: { run: (namespace: object) => void }) {
  props.run(helpers);
  return <div />;
}
