import * as hand from "./hand";

// fnA and fnB are handed out by name (a static member read used as a value); fnC is not.
export const handed = [hand.fnA, hand.fnB];

export function Comp() {
  return <i />;
}
