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
