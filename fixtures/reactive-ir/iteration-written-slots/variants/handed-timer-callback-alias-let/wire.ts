// ADR 1013, negative: the alias names a `let` two writes may fill, one of them a function that reads its `this`: what it holds is not one function.
let pick: (this: unknown) => void = () => {};
if (Math.random() > 2) {
  pick = function (this: unknown) {
    console.log(this);
  };
}
const again = pick;
setTimeout(again, 0);
export {};
