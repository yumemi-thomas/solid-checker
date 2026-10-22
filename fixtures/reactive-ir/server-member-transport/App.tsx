import { loadTyped, loadUser } from "./api";

// Unresolved member of a server function: whether this call crosses the
// transport is unproven, so the checker is uncertain about the Date argument.
export function Unresolved(props: { when: Date }) {
  loadUser.keyFor(props.when);
}

// Resolved member: `keyFor` is a method declared on the wrapper, not the server
// function, so the Date argument never travels to the server. Silent.
export function Resolved(props: { when: Date }) {
  loadTyped.keyFor(props.when);
}
