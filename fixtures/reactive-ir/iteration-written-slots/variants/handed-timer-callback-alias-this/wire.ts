// ADR 1013, negative: the alias names a function that reads its `this`, which a timer makes the window: the global object reaches code that may hand it on.
function tick(this: unknown) {
  console.log(this);
}
const again = tick;
setTimeout(again, 0);
export {};
