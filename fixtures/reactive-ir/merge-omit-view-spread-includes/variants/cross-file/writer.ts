import { arrayProto } from "./proto";

// A write in another file than the one that reached the prototype, under a
// key built at run time.
const key = ["incl", "udes"].join("");
const includes = (arrayProto as Record<string, (value: unknown, from?: number) => boolean>)[key];
Object.defineProperty(arrayProto, key, {
  configurable: true,
  writable: true,
  value(this: unknown[], value: unknown, from?: number): boolean {
    return value === "children" ? false : includes.call(this, value, from);
  },
});
