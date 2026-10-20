import { Dynamic } from "@solidjs/web";

// A project read of a component key: the value read back could be called by
// the project, which the census does not see, so the premise is not applied.
export function Frame(props: { component: () => unknown }) {
  return <Dynamic component={props.component} />;
}
