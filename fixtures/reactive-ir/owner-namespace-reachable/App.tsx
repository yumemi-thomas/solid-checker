import * as member from "./member";
import * as objects from "./objects";
import { namedValue } from "./member";

// `member.handedOut` and `member.handedOutDirect` are read as values here,
// never called.
export const values = [member.handedOut, member.handedOutDirect];

// The namespace object itself is handed out, so every export of `./objects` is
// reachable from whatever receives these entries.
export const everything = Object.entries(objects);

// A named import used as a value. Not a namespace hand-out.
export const named = namedValue;

// A namespace call from a component: the caller is known.
export function Host() {
  member.setup();
  return <i />;
}
