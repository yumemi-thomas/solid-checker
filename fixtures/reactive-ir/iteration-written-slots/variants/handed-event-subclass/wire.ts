let kept: any;
class E extends Event {
  constructor() {
    super("x");
    kept = this;
  }
}
window.dispatchEvent(new E());
const g = kept.target;
const AP = g.Array.prototype;
const keys: PropertyKey[] = [Symbol.iterator];
AP[keys[0]] = function () {};
export {};
