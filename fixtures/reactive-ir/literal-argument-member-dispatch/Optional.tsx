import { createSignal } from "solid-js";
import { optionalWords, optionalStamp, optionalFill } from "./source";

declare const input: string | undefined;
declare const date: Date | undefined;

export function OptionalPrimitive() {
  const words = optionalWords(input);
  return <div>{words}</div>;
}

export function OptionalObject() {
  const time = optionalStamp(date);
  return <div>{time}</div>;
}

export function OptionalCallback() {
  const [name] = createSignal("you");
  const text = optionalFill("Hello {name}", () => name());
  return <div>{text}</div>;
}
