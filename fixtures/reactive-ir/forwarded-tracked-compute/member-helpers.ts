import { createMemo } from "solid-js";
import { escapedSink, freshSink, mixedSink, sampledSink, siblingSink, trackedSink } from "./member-consumers";

type Options = { read: () => number };
declare function keep(read: () => number): void;
declare function stashReceiver(value: unknown): void;

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

export function routedMember(options: Options) {
  return trackedSink({ compute: () => options.read() });
}
export function mixedRoutedMember(options: Options) {
  return mixedSink({ compute: () => options.read() });
}
export function freshRoutedMember(options: Options) {
  freshSink({ compute: () => options.read() });
}
export function sampledRoutedMember(options: Options) {
  return sampledSink({ compute: () => options.read() });
}
export function escapedRoutedMember(options: Options) {
  escapedSink({ compute: () => options.read() });
}

export function siblingRoutedMember(options: Options) {
  return siblingSink({ compute: () => options.read(), sibling: () => 0 });
}
export function receiverRoutedMember(options: Options) {
  return siblingSink({ compute: () => options.read(), sibling() { return this.compute(); } });
}

export function subjectReceiverRoutedMember(options: Options) {
  return trackedSink({ compute: function () {
    stashReceiver(this);
    return options.read();
  } });
}
