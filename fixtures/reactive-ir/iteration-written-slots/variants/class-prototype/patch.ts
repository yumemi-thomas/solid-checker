class Foo {
  bar = 0;
}
(Foo.prototype as unknown as { bar: number }).bar = 1;
export {};
