import { Counter, Derived, Gadget, Widget, bump, contains, poke, run, stamp } from "./source";

// Clean: no write through a prototype reaches `Counter`'s chain, so
// `Counter.bump` is the method that runs, and it reads nothing.
export function ExactClass() {
  const counter = new Counter();
  const value = bump(counter);
  return <div>{value}</div>;
}

// Clean: a `Date` and a `Set` built here inherit only ECMAScript prototypes,
// which no write through a project or DOM prototype reaches.
export function BuiltinInstances() {
  const found = contains(new Set(["a"]), "a");
  const time = stamp(new Date(0));
  return <div>{time + Number(found)}</div>;
}

// Uncertifiable: `Widget.prototype` is written at a key no proof names.
export function WrittenClass() {
  const value = poke(new Widget());
  return <div>{value}</div>;
}

// Uncertifiable: `Gadget`'s chain holds `Widget.prototype`.
export function WrittenBase() {
  const value = poke(new Gadget());
  return <div>{value}</div>;
}

// Uncertifiable: `Derived` extends a call, which no class resolves.
export function ExtendsConstructor() {
  const value = run(new Derived());
  return <div>{value}</div>;
}
