import { flush } from "solid-js";
import { Dynamic } from "@solidjs/web";
import { Frame, runStep } from "steplib";

// Batch 11, the review of #217 (causes A to E): a context places a value only when it CARRIES it. The program names no action, effect or leaf primitive, so the program censuses say
// the three flush codes cannot arise; `steplib` (its source is not read: its manifest names no Solid package) calls `action` itself and runs what it is handed as a step. Each
// callback below is handed to it by a route #217's build read as "placed in a binding / an assignment / an argument / a child" (the context merely CONTAINED the callback: a
// receiver, a template part, a `yield`, an `await`) or by a route it did not follow (`arguments`, a returned function's call, an intrinsic element's `ref`): its flush keeps its
// three uncertifiable rows (`flush-in-action-unaccounted`, `flush-in-effect-unaccounted`, `leaf-operation-unaccounted`), never silence, never a violation. Each line is in a timer's
// callback. The last section (E) is the other way: a component's closing tag, answered like its opening one, stops handing a component written with children.

// ---- A: an array literal that is a receiver, inside a context that "held" it
const mapInit = () => {
  flush();
};
setTimeout(() => {
  const rs = [mapInit].map((f) => runStep(f));
});

const forEachInit = () => {
  flush();
};
setTimeout(() => {
  const n = [forEachInit].forEach((f) => {
    void runStep(f);
  });
});

const mapDestr = () => {
  flush();
};
const destrList = [mapDestr];
const [destrFirst] = destrList;
setTimeout(() => {
  const rs = [destrFirst].map((f) => runStep(f));
});

const mapParam = () => {
  flush();
};
function viaParam(g: () => void) {
  const rs = [g].map((f) => runStep(f));
}
setTimeout(() => {
  viaParam(mapParam);
});

const mapFlat = () => {
  flush();
};
setTimeout(() => {
  const rs = [[mapFlat]].flat().map((f) => runStep(f));
});

const mapTemplate = () => {
  flush();
};
setTimeout(() => {
  const text = `${[mapTemplate].map((f) => runStep(f)).length}`;
});

const mapLogical = () => {
  flush();
};
setTimeout(() => {
  const ok =
    { a: mapLogical }.a.name.length > 0 &&
    [mapLogical].some((f) => {
      void runStep(f);
      return true;
    });
});

const mapAwait = () => {
  flush();
};
setTimeout(async () => {
  await [mapAwait].map((f) => runStep(f))[0];
});

const mapAssign = () => {
  flush();
};
let assigned: unknown;
setTimeout(() => {
  assigned = [mapAssign].map((f) => runStep(f));
});

const mapArg = () => {
  flush();
};
function use(value: unknown) {
  return value;
}
setTimeout(() => {
  use([mapArg].map((f) => runStep(f)));
});

const mapJsx = () => {
  flush();
};
export function MapJsx() {
  return (
    <div>
      {[mapJsx].map((f) => {
        void runStep(f);
        return 1;
      })}
    </div>
  );
}

const yielded = () => {
  flush();
};
function* generate(): Generator<() => void, void, undefined> {
  const x = yield yielded;
}
setTimeout(() => {
  for (const f of generate()) void runStep(f);
});

// ---- A, the review's N-B: the step the package runs calls the literal's elements (the literal is a receiver there too, so #217's build said "not handed")
const mapCalled = () => {
  flush();
};
function stepMap() {
  const rs = [mapCalled].map((f) => f());
}
setTimeout(() => {
  void runStep(stepMap);
});

const objectCalled = () => {
  flush();
};
function stepObject() {
  const r = { run: objectCalled }.run();
}
setTimeout(() => {
  void runStep(stepObject);
});

// ---- B: a project function that names `arguments` reads every argument through it
const argsIndex = () => {
  flush();
};
function readsArguments(_f: () => void) {
  void runStep(arguments[0]);
}
setTimeout(() => {
  readsArguments(argsIndex);
});

const argsApply = () => {
  flush();
};
function takesRest(...fs: Array<() => void>) {
  void runStep(fs[0]);
}
function forwardsArguments(_f: () => void) {
  takesRest.apply(null, arguments as unknown as Array<() => void>);
}
setTimeout(() => {
  forwardsArguments(argsApply);
});

// ---- C: a function written as an argument returns the value to the call
const thenReturn = () => {
  flush();
};
setTimeout(() => {
  void Promise.resolve()
    .then(() => thenReturn)
    .then((f) => runStep(f));
});

// ---- D: an intrinsic element's `ref` callback is applied where the element is created (rc.21 `@solidjs/web` applies `ref` at once, on the stack of the code that makes the
// element), so a step the package runs that writes `<div ref={cb} />` runs the callback; so does `Dynamic` with a string tag, which creates the element it names
const refProp = () => {
  flush();
};
function stepRef() {
  const el = <div ref={refProp} />;
}
setTimeout(() => {
  void runStep(stepRef);
});

const refDynamic = () => {
  flush();
};
function stepDynamic() {
  const el = <Dynamic component="div" ref={refDynamic} />;
}
setTimeout(() => {
  void runStep(stepDynamic);
});

// ---- the control: handed to the package by name (#216's own route)
const control = () => {
  flush();
};
setTimeout(() => {
  void runStep(control);
});

// ---- silent twins: the context carries the callback, and the facts of the program show it goes nowhere a package runs it
const twinConditional = () => {
  flush();
};
const twinOther = () => {};
const keptNowhere = Math.random() > 0.5 ? twinConditional : twinOther;

const twinLogical = () => {
  flush();
};
const keptLogical = twinLogical || twinOther;

const twinObject = () => {
  flush();
};
const keptObject = { run: twinObject };

const twinParameter = () => {
  flush();
};
function readsDeclared(f: () => void) {
  void f;
}
setTimeout(() => {
  readsDeclared(twinParameter);
});

const twinThen = () => {
  flush();
};
setTimeout(() => {
  void Promise.resolve().then(() => twinThen);
});

// the same element written in a function nothing runs: the callback goes nowhere a package runs it
const twinRef = () => {
  flush();
};
function neverRuns() {
  const el = <div ref={twinRef} />;
}

// ---- E (Mac 4's finding on `position`): the closing tag's name is the same reference of the component as the opening one, and answered the same way. A component written with
// children is not handed to the package for its closing tag (#217's build answered the opening name only: `<Card>x</Card>` handed Card, `<Card />` did not)
function WithChildren(props: { children?: unknown }) {
  flush();
  return <div>{props.children as never}</div>;
}
function Bare() {
  flush();
  return <div />;
}
export function Uses() {
  return (
    <div>
      <WithChildren>x</WithChildren>
      <Bare />
    </div>
  );
}

// the same pair rendered by a function the package runs: both are handed (the answer is the render site's, whichever way the tag is written)
function HandedChildren(props: { children?: unknown }) {
  flush();
  return <div>{props.children as never}</div>;
}
function HandedBare() {
  flush();
  return <div />;
}
function RendersBoth() {
  return (
    <div>
      <HandedChildren>x</HandedChildren>
      <HandedBare />
    </div>
  );
}
setTimeout(() => {
  void runStep(RendersBoth as unknown as () => void);
});

// C by reference: const, with a discarded-result twin.
const returnedConst = () => { flush(); };
const giveConst = () => returnedConst;
setTimeout(() => { void Promise.resolve().then(giveConst).then(f => runStep(f)); });
const twinReturnedConst = () => { flush(); };
const twinGiveConst = () => twinReturnedConst;
setTimeout(() => { void Promise.resolve().then(twinGiveConst); });

// C by reference: let, with a discarded-result twin.
const returnedLet = () => { flush(); };
let giveLet = () => returnedLet;
setTimeout(() => { void Promise.resolve().then(giveLet).then(f => runStep(f)); });
const twinReturnedLet = () => { flush(); };
let twinGiveLet = () => twinReturnedLet;
setTimeout(() => { void Promise.resolve().then(twinGiveLet); });

// C by reference: declaration, with a discarded-result twin.
const returnedDeclaration = () => { flush(); };
function giveDeclaration() { return returnedDeclaration; }
setTimeout(() => { void Promise.resolve().then(giveDeclaration).then(f => runStep(f)); });
const twinReturnedDeclaration = () => { flush(); };
function twinGiveDeclaration() { return twinReturnedDeclaration; }
setTimeout(() => { void Promise.resolve().then(twinGiveDeclaration); });

// C by reference: member, with a discarded-result twin.
const returnedMember = () => { flush(); };
const boxMember = { give: () => returnedMember };
setTimeout(() => { void Promise.resolve().then(boxMember.give).then(f => runStep(f)); });
const twinReturnedMember = () => { flush(); };
const twinBoxMember = { give: () => twinReturnedMember };
setTimeout(() => { void Promise.resolve().then(twinBoxMember.give); });

// ---- the review of #222, B1: an assignment expression's own value is the value it writes, so the binding, the target and the receiver it is written in take it on
const chainConst = () => { flush(); };
let chainLast: unknown;
setTimeout(() => {
  const first = (chainLast = chainConst);
  void runStep(first);
});
const chainAssign = () => { flush(); };
let chainFirst: () => void = () => {};
let chainOther: unknown;
setTimeout(() => {
  chainFirst = chainOther = chainAssign;
  void runStep(chainFirst);
});
const chainReceiver = () => { flush(); };
let chainHeld: unknown;
setTimeout(() => {
  const rs = [(chainHeld = chainReceiver)].map((f) => runStep(f));
});
const chainNullish = () => { flush(); };
let chainMaybe: (() => void) | undefined;
setTimeout(() => {
  void runStep((chainMaybe ??= chainNullish));
});
// twin: the binding and the target hold it, and nothing reads either
const twinChain = () => { flush(); };
let twinChainLast: unknown;
setTimeout(() => {
  const kept = (twinChainLast = twinChain);
});

// ---- the review of #222, D: an intrinsic element applies a spread's `ref` and inserts its children (a function child is called) where it is created
const spreadRef = () => { flush(); };
function stepSpread() {
  const el = <div {...{ ref: spreadRef }} />;
}
setTimeout(() => { void runStep(stepSpread); });
const spreadHeld = () => { flush(); };
const spreadProps = { ref: spreadHeld };
function stepSpreadHeld() {
  const el = <div {...spreadProps} />;
}
setTimeout(() => { void runStep(stepSpreadHeld); });
const childFunction = () => { flush(); };
function stepChild() {
  const el = <div>{childFunction as never}</div>;
}
setTimeout(() => { void runStep(stepChild); });
// a spread's keys are not read: the same `onClick` written in a spread keeps its rows
const spreadOther = () => { flush(); };
function stepSpreadOther() {
  const el = <div {...{ onClick: spreadOther }} />;
}
setTimeout(() => { void runStep(stepSpreadOther); });
// twins: an attribute the compiler records as an event handler (the DOM runs it, on its own stack), and a spread and a child in a function nothing runs
const twinListener = () => { flush(); };
function stepListener() {
  const el = <div onClick={twinListener} />;
}
setTimeout(() => { void runStep(stepListener); });
const twinSpreadRef = () => { flush(); };
const twinChildFunction = () => { flush(); };
function neverCreates() {
  const el = <div {...{ ref: twinSpreadRef }}>{twinChildFunction as never}</div>;
}

// ---- the review of #222, E: a component written with children under an unread component's children; both tag names get the render site's answer
function UnderFrame(props: { children?: unknown }) {
  flush();
  return <div>{props.children as never}</div>;
}
setTimeout(() => {
  void (<Frame><UnderFrame>x</UnderFrame></Frame>);
});
// twin: the same element written as a child of an intrinsic element in a timer's callback: created there, where no package runs it
function TwinUnderDiv(props: { children?: unknown }) {
  flush();
  return <div>{props.children as never}</div>;
}
setTimeout(() => {
  void (<div><TwinUnderDiv>{"x"}</TwinUnderDiv></div>);
});
