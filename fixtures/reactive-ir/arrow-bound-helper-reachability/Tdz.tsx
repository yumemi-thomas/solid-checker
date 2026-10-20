import { action, createRoot, createSignal } from "solid-js";

const [count, setCount] = createSignal(0);

// Negative: `go` is a hoisted declaration, and the root below runs it while
// module evaluation is still above `late`'s declarator: `late` is in its
// temporal dead zone, the call throws ReferenceError, and the setter never
// runs. `tsc` accepts the early use inside a function body.
function go() {
  late();
}
createRoot(() => {
  go();
});
const late = () => {
  setCount(1);
};

// Negative: `run` is created before `helper`'s declarator and called before
// it; `helper` is in its temporal dead zone.
export function NestedBeforeDeclarator() {
  const run = () => helper();
  run();
  const helper = () => {
    setCount(2);
  };
  return <button>nested before</button>;
}

// Negative: the same early call of an action helper.
export function NestedActionBeforeDeclarator() {
  const go = () => actionHelper();
  go();
  const actionHelper = () => {
    const run = action(function* () {});
    void run();
  };
  return <button>nested action before</button>;
}

// Positive control: a nested declaration is hoisted, so the same early call
// reaches it.
export function NestedHoistedDeclaration() {
  const run = () => hoisted();
  run();
  function hoisted() {
    setCount(3);
  }
  return <button>nested hoisted</button>;
}

// Positive: the arrow is created after the declarator, in the same body.
export function CreatedAfterDeclarator() {
  const helper = () => {
    setCount(4);
  };
  const run = () => helper();
  run();
  return <button>created after</button>;
}

// Positive: `run` is created before the declarator, but every mention of it
// follows the declarator, so it cannot run before `helper` exists.
export function CreatedBeforeMentionedAfter() {
  const run = () => helper();
  const helper = () => {
    setCount(11);
  };
  run();
  return <button>mentioned after</button>;
}

// Positive: a nested declaration is hoisted, but every mention of it follows
// the declarator, so it cannot run before `helper` exists.
export function DeclarationMentionedAfter() {
  function run() {
    helper();
  }
  const helper = () => {
    setCount(12);
  };
  run();
  return <button>declaration after</button>;
}

// Negative: the same declaration is mentioned before the declarator.
export function DeclarationMentionedBefore() {
  function run() {
    helper();
  }
  run();
  const helper = () => {
    setCount(13);
  };
  return <button>declaration before</button>;
}

// Negative: a `var` reads undefined before its declarator, so calling it
// throws TypeError and the body never runs.
export function VarBeforeDeclarator() {
  const run = () => viaVar();
  run();
  var viaVar = () => {
    setCount(5);
  };
  return <button>var before</button>;
}

// Negative: a conditional initializer is not one function; which arrow the
// call reaches is unknown.
const choose = (flag: boolean) => {
  const picked = flag ? () => setCount(6) : () => undefined;
  return picked;
};
export function ConditionalInitializer() {
  const picked = choose(Math.random() > 0.5);
  picked();
  return <button>conditional</button>;
}

// Negative: a destructured function is not an exact binding of the arrow.
const { destructured } = { destructured: () => setCount(7) };
export function Destructured() {
  destructured();
  return <button>destructured</button>;
}

// Negative: a call result holds whatever the wrapper returns, not the arrow
// handed to it.
declare function wrap<T extends () => void>(fn: T): () => void;
const wrapped = wrap(() => setCount(8));
export function WrappedInitializer() {
  wrapped();
  return <button>wrapped</button>;
}

// Positive: a module-level declaration calls an arrow declared earlier, and
// nothing runs ahead of the declarator that could call the declaration.
const early = () => {
  setCount(9);
};
export function HoistedCaller() {
  early();
  return <button>hoisted caller</button>;
}

export function Count() {
  return <button>{count()}</button>;
}
