function plain(_g: unknown) {}
const arrow = (_g: unknown) => {};
const o = { m(_g: unknown) {} };
class A {
  m(_g: unknown) {}
}
const a = new A();
plain(window);
arrow(window);
o.m(window);
a.m(window);
plain.call(null, window);
function go() {
  o.m(window);
}
go();
export {};
