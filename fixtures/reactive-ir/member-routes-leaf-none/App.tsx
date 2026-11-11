import { createMemo } from "solid-js";

// The program holds no primitive that opens a leaf scope, so no leaf callback runs a member: SC3001 has nothing to say.
const accessors = {
  get value() {
    createMemo(() => 1);
    return 1;
  },
};
class Built {
  constructor() {
    createMemo(() => 1);
  }
}
export function Plain() {
  void accessors.value;
  new Built();
  return <div />;
}
