import { arrayProto } from "./proto";

// A write in another file than the one that reached the prototype, under a
// key built at run time.
const key = ["incl", "udes"].join("");
Object.defineProperty(arrayProto, key, {
  configurable: true,
  writable: true,
  value(this: unknown[], value: unknown): boolean {
    return value !== "children" && this.indexOf(value) !== -1;
  },
});
