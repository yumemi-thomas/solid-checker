import { createSignal } from "solid-js";
import { fill, label, shout, stamp, translate, words, type Replaceable } from "./source";

declare const userName: string;
declare const lastSeen: Date;

// Negative: a string literal's `replace` is `String.prototype.replace`.
export function StringLiteral() {
  const text = translate("Hello {name}");
  return <div>{text}</div>;
}

// Negative: so is a template literal's.
export function TemplateLiteral() {
  const text = translate(`Bye {name}`);
  return <div>{text}</div>;
}

// Control: an object whose `replace` is its own: resolved through the
// object, as before.
export function ObjectArgument() {
  const text = translate(label);
  return <div>{text}</div>;
}

// Control: a caller-supplied value whose implementation cannot be selected
// stays a proof obligation.
export function UnknownArgument(props: { value: Replaceable }) {
  const text = translate(props.value);
  return <div>{text}</div>;
}

// The literal settles which `replace` runs, not what the callee's own
// replacer reads: `fill` reads `name()` synchronously while it runs.
export function LiteralWithReadingReplacer() {
  const [name] = createSignal("you");
  const text = fill("Hello {name}", name);
  return <div>{text}</div>;
}

// Negative: `shout`'s member call is `String.toUpperCase`, so a non-literal
// string argument selects the built-in too.
export function TypedString() {
  const text = shout(userName);
  return <div>{text}</div>;
}

// Control: `Date.getTime` is not a primitive built-in; the obligation stays.
export function ObjectBuiltin() {
  const time = stamp(lastSeen);
  return <div>{time}</div>;
}

// Negative: `message.trim()` is `String.trim` even inside a chain.
export function ChainedBuiltin() {
  const parts = words(userName);
  return <div>{parts}</div>;
}
