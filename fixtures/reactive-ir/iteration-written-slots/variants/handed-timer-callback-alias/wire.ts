// ADR 1013: a `const` that names another binding holds that binding's function: the timer calls an arrow, which has no `this`, so the window reaches no code.
const run = () => {};
const again = run;
setTimeout(again, 0);
export {};
