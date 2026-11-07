class A {
  m(_g: any) {}
}
class B extends A {
  m(g: any) {
    const AP = g.Array.prototype;
  const keys: PropertyKey[] = [Symbol.iterator];
  AP[keys[0]] = function () {};
  }
}
const a: A = new B();
a.m(globalThis);
export {};
