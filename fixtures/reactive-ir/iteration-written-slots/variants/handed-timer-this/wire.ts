function tick(this: unknown) {
  console.log(this);
}
setTimeout(tick, 0);
export {};
