import { flush, untrack } from "solid-js";
import { Dynamic } from "@solidjs/web";
import { Pkg, config, runAll, runConfig, runGet, runNs, runStep, runWith } from "steplib";
import * as Local from "./nslocal";
import * as NS from "./nsmod";

// Batch 11, the flip of ADR 0967's default (the review of #216): a value counts as handed to a package nothing here reads unless a fact of the program proves where it
// goes. The program names no action, no effect and no leaf primitive, so the program censuses say the three codes cannot arise; but `steplib` (its source is not read: its
// manifest names no Solid package) calls `action` itself and runs what it is handed as a step. Each function below that reaches it is three uncertifiable rows (`flush-in-action-
// unaccounted`, `flush-in-effect-unaccounted`, `leaf-operation-unaccounted`); the twins that go where the program shows (a DOM property, a project function's own call, a
// namespace that is only called into) stay silent. The boundary is closed (`.solid-checker/runtime.json`), so an export is entered by project call sites only.

// ---- a value kept in a collection and read elsewhere, an iterable, a receiver array, a promise's value
const kept = new Set<() => void>();
kept.add(() => {
  flush();
});
export function Collection() {
  setTimeout(() => {
    kept.forEach((f) => {
      void runStep(f);
    });
  });
  return <p>x</p>;
}

export function ForOf() {
  setTimeout(() => {
    for (const f of [
      () => {
        flush();
      },
    ]) {
      void runStep(f);
    }
  });
  return <p>x</p>;
}

export function ReceiverArray() {
  setTimeout(() => {
    [
      () => {
        flush();
      },
    ].forEach((f) => {
      void runStep(f);
    });
  });
  return <p>x</p>;
}

export function PromiseValue() {
  setTimeout(() => {
    void Promise.resolve(() => {
      flush();
    }).then((f) => {
      void runStep(f);
    });
  });
  return <p>x</p>;
}

// ---- a method of a class whose instance is handed: a class's functions are handed whole (an instance, a subclass or `this` may hand them from where nothing here follows)
class Job {
  run() {
    flush();
  }
}
export function ClassMethod() {
  setTimeout(() => {
    void runWith(new Job());
  });
  return <p>x</p>;
}

// ---- an assignment into an object a package exports (handed), and into a local element's property (followed through the element's reads: none here)
export function PackageAssign() {
  setTimeout(() => {
    config.onStep = () => {
      flush();
    };
    void runConfig();
  });
  return <p>x</p>;
}

export function DomProperty() {
  const el = document.createElement("button");
  el.onclick = () => {
    flush();
  };
  return <p>x</p>;
}

// ---- a project component's props: followed in the component's own reads (the first parameter); one that goes on to the package is handed, one the component only calls is not
function Wrap(props: { render: () => void }) {
  return <Pkg render={props.render} />;
}
export function ComponentProps() {
  return (
    <Wrap
      render={() => {
        flush();
      }}
    />
  );
}

function Own(props: { render: () => void }) {
  props.render();
  return <p>x</p>;
}
export function ComponentPropsCalled() {
  return (
    <Own
      render={() => {
        flush();
      }}
    />
  );
}

// ---- what a handed function returns (the package calls it), and a reflective call whose target is a project function that goes on to the package
const accessed = () => {
  flush();
};
export function Accessor() {
  setTimeout(() => {
    void runGet(() => accessed);
  });
  return <p>x</p>;
}

function viaStep(run: () => void) {
  void runStep(run);
}
export function ReflectedProject() {
  setTimeout(() => {
    viaStep.call(null, () => {
      flush();
    });
  });
  return <p>x</p>;
}

// ---- a callback run inline by known code on the stack of a step a package runs: the standard library, a project function's parameter, a dialect primitive
const innerStandard = () => {
  flush();
};
function stepStandard() {
  [1].forEach(innerStandard);
}
export function InlineStandard() {
  setTimeout(() => {
    void runStep(stepStandard);
  });
  return <p>x</p>;
}

const innerParameter = () => {
  flush();
};
function callIt(f: () => void) {
  f();
}
function stepParameter() {
  callIt(innerParameter);
}
export function InlineParameter() {
  setTimeout(() => {
    void runStep(stepParameter);
  });
  return <p>x</p>;
}

const innerPrimitive = () => {
  flush();
};
function stepPrimitive() {
  untrack(innerPrimitive);
}
export function InlinePrimitive() {
  setTimeout(() => {
    void runStep(stepPrimitive);
  });
  return <p>x</p>;
}

// ---- the same inline runs on a stack no package runs, a call through `call`, a DOM listener's registration and an operand nothing takes: silent
const quiet = () => {
  flush();
};
function callQuietly(f: () => void) {
  f();
}
function quietStep() {
  [1].forEach(quiet);
  callQuietly(quiet);
  untrack(quiet);
}
export function InlineLocal() {
  setTimeout(() => {
    quietStep();
    quiet.call(null);
  });
  return <p>x</p>;
}

export function Listener() {
  const el = document.createElement("button");
  el.addEventListener("click", quiet);
  el.removeEventListener("click", quiet);
  return <p>x</p>;
}

export function Inert() {
  if (quiet === undefined) {
    return null;
  }
  return <p>x</p>;
}

// ---- `<Dynamic component={…}>` hands its props to what `component` names: a package's component is handed, a string names an intrinsic element (the DOM's)
export function DynamicPackage() {
  return (
    <Dynamic
      component={Pkg}
      render={() => {
        flush();
      }}
    />
  );
}

export function DynamicIntrinsic() {
  return (
    <Dynamic
      component="button"
      onClick={() => {
        flush();
      }}
    />
  );
}

// ---- a namespace object handed whole hands each export of the module (`nsmod.ts`); one only called into hands nothing (`nslocal.ts`)
export function Namespaced() {
  setTimeout(() => {
    void runNs(NS);
  });
  return <p>x</p>;
}

export function NamespaceCalled() {
  setTimeout(() => {
    Local.localStep();
  });
  return <p>x</p>;
}

// ---- a package's list of callbacks, by the collection route of the earlier fixture (`flush-universe-package-hand-off`), kept here as the control of the new default
export function Control() {
  setTimeout(() => {
    void runAll([
      () => {
        flush();
      },
    ]);
  });
  return <p>x</p>;
}

// ---- an export nothing in the program enters: with the boundary closed the site never runs (silent); the `open` variant is the same function with an open boundary
export function Unreferenced() {
  flush();
}
