// One export per invoking form the implementation call census does not
// record, plus the two controls. See README.md for which marker kinds live
// here and which cannot.

export function taggedForm(tag) {
  return tag`plain`;
}

class Box {
  constructor() {
    this.plain = 2;
  }
  get value() {
    return 1;
  }
  set value(next) {
    void next;
  }
}

const box = new Box();

export function getAccessorForm() {
  return box.value;
}

export function setAccessorForm() {
  box.value = 3;
}

export function plainMemberForm() {
  return box.plain;
}

export function unknownMemberForm(bag) {
  return bag.whatever;
}

export function spreadForm(values) {
  return [...values];
}

export function forOfForm(values) {
  let total = 0;
  for (const value of values) {
    total = total + value;
  }
  return total;
}

export function instanceofForm(value) {
  return value instanceof Error;
}

export async function awaitThenableForm(value) {
  return await value;
}

export function coercionForm(value) {
  return `value: ${value}`;
}

export function objectSpreadForm(options) {
  return { ...options };
}

// The nested case: the tagged template sits inside a callable the export hands
// back, so its marker carries that callable and captured. Lexical containment
// in a returned closure is not execution.
export function capturedTaggedForm(tag) {
  return () => tag`captured`;
}

// The control. Its body holds one plain call and nothing else, so its marker
// list is present and empty — the producer's positive claim that every form it
// walked was a call, a construction, or provably non-invoking.
export function plainCallForm(callback) {
  callback();
}
