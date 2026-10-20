export interface RootProps { children?: import("@solidjs/web").JSX.Element }
export function Root(props: { children?: import("@solidjs/web").JSX.Element }) {
  return props.children;
}
export function readNow(read: () => number) {
  return read();
}
export function readLater(read: () => number) {
  queueMicrotask(() => read());
}
export function viaThis(this: any, read: () => number) {
  return this ? this.readNow(read) : 0;
}
Root.Child = function Child(props: { read: () => number }) {
  return props.read();
};
export function other(read: () => number) {
  return read();
}
export function viaThisOpt(this: any, read: () => number) {
  this?.other?.(read);
  return 0;
}
export function viaThisTypeof(this: any, read: () => number) {
  if (typeof this === "object" && this !== null) this.other(read);
  return 0;
}
