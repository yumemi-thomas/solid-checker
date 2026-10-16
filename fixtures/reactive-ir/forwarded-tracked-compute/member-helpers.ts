import { createMemo } from "solid-js";

type Options = { read: () => number };
declare function keep(read: () => number): void;

export function trackedMember(options: Options) {
  return createMemo(() => options.read());
}

export function mixedMember(options: Options) {
  options.read();
  return createMemo(() => options.read());
}

export function deferredMember(options: Options) {
  setTimeout(() => { options.read(); }, 0);
}

export function unknownMember(options: Options) {
  keep(() => options.read());
}

export function defaultMember(options: Options) {
  return createMemo(() => {
    keep((value = options.read()) => value);
    return 0;
  });
}
