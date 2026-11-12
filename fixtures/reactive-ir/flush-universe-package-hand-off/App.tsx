import { flush } from "solid-js";
import * as steps from "steplib";
import { Pkg, runAll, runStep, runStep as go, runWith } from "steplib";
import { holder, shared } from "./Shared";

// Batch 11, the B3 PR after #213 (ADR 0967): a flush in a function an installed package the build does not read as source may run gets no census reason. The program
// names no action, no effect and no leaf primitive, so the program censuses say the three codes cannot arise; but `steplib` (its source is not read: its manifest names
// no Solid package) calls `action` itself, and runs what it is handed as a step. Each function handed to it, by every route, is three uncertifiable rows (`flush-in-action-
// unaccounted`, `flush-in-effect-unaccounted`, `leaf-operation-unaccounted`); the sites that no package can run stay silent on the census reasons.

// ---- by argument: the function written where it is handed
export function Literal() {
  setTimeout(() => {
    void runStep(() => {
      flush();
    });
  });
  return <p>x</p>;
}

// ---- by reference: a function declaration, a const copy, an import
function byReference() {
  flush();
}
export function Reference() {
  setTimeout(() => {
    void runStep(byReference);
  });
  return <p>x</p>;
}

const original = () => {
  flush();
};
const copy = original;
export function Copy() {
  setTimeout(() => {
    void runStep(copy);
  });
  return <p>x</p>;
}

let kept: () => void = () => {};
kept = () => {
  flush();
};
export function Assigned() {
  setTimeout(() => {
    void runStep(kept);
  });
  return <p>x</p>;
}

export function Imported() {
  setTimeout(() => {
    void runStep(shared);
    void runWith(holder);
  });
  return <p>x</p>;
}

// ---- object members and array elements
export function Member() {
  setTimeout(() => {
    void runWith({
      run: () => {
        flush();
      },
    });
  });
  return <p>x</p>;
}

export function Method() {
  setTimeout(() => {
    void runWith({
      run() {
        flush();
      },
    });
  });
  return <p>x</p>;
}

export function Spread() {
  setTimeout(() => {
    void runWith({
      ...{
        run: () => {
          flush();
        },
      },
    });
  });
  return <p>x</p>;
}

const opts = {
  run: () => {
    flush();
  },
};
export function Holder() {
  setTimeout(() => {
    void runWith(opts);
  });
  return <p>x</p>;
}

export function MemberByReference() {
  setTimeout(() => {
    void runWith({ run: byReference });
  });
  return <p>x</p>;
}

export function Element() {
  setTimeout(() => {
    void runAll([
      () => {
        flush();
      },
    ]);
  });
  return <p>x</p>;
}

const steps = [
  () => {
    flush();
  },
];
export function ElementHolder() {
  setTimeout(() => {
    void runAll(steps);
  });
  return <p>x</p>;
}

// ---- JSX: a prop, a spread attribute, a child expression and a prop by reference of a component of the package
export function Prop() {
  return (
    <Pkg
      render={() => {
        flush();
      }}
    />
  );
}

export function PropByReference() {
  return <Pkg render={byReference} />;
}

export function PropSpread() {
  return (
    <Pkg
      {...{
        render: () => {
          flush();
        },
      }}
    />
  );
}

export function Children() {
  return (
    <Pkg>
      {() => {
        flush();
      }}
    </Pkg>
  );
}

// ---- through a project function: its parameter goes on to the package, and a helper the handed callback calls
function wrap(run: () => void) {
  void runStep(run);
}
export function Wrapped() {
  setTimeout(() => {
    wrap(() => {
      flush();
    });
  });
  return <p>x</p>;
}

function helper() {
  flush();
}
export function Caller() {
  setTimeout(() => {
    void runStep(() => helper());
  });
  return <p>x</p>;
}

// ---- a function a project function returns (a call evaluates to it) and a reflective call that forwards its arguments to the package's function
function make() {
  return () => {
    flush();
  };
}
export function Returned() {
  setTimeout(() => {
    void runStep(make());
  });
  return <p>x</p>;
}

const immediate = (() => () => {
  flush();
})();
export function Immediate() {
  setTimeout(() => {
    void runStep(immediate);
  });
  return <p>x</p>;
}

export function Reflected() {
  setTimeout(() => {
    void runStep.call(null, () => {
      flush();
    });
    void Reflect.apply(runStep, null, [
      () => {
        flush();
      },
    ]);
    void runStep.bind(null, () => {
      flush();
    })();
  });
  return <p>x</p>;
}

// ---- the package named another way: through a namespace import and through an alias (the callee is resolved by its declaration, never by its spelling)
export function Namespace() {
  setTimeout(() => {
    void steps.runStep(() => {
      flush();
    });
  });
  return <p>x</p>;
}

export function Aliased() {
  setTimeout(() => {
    void go(() => {
      flush();
    });
  });
  return <p>x</p>;
}

// ---- a local function that shadows the package's name is a project function (analyzed, silent): the spelling `runStep` names the declaration that is in scope
export function Shadowed() {
  const runStep = (cb: () => void) => {
    cb();
  };
  setTimeout(() => {
    runStep(() => {
      flush();
    });
  });
  return <p>x</p>;
}

// ---- silent: nothing a package does can run these on the census's reasons
// The standard library runs a callback as the program's own code (a scheduler, an inline carrier, a promise), a DOM element's handler is the browser's, and a project function
// that only calls its parameter is analyzed.
export function Known() {
  setTimeout(() => {
    [1].forEach(() => {
      flush();
    });
    void Promise.resolve().then(() => {
      flush();
    });
    call(() => {
      flush();
    });
  });
  return (
    <button
      onClick={() => {
        flush();
      }}
    >
      x
    </button>
  );
}

function call(run: () => void) {
  run();
}

// A function nothing hands to a package: only called.
function plain() {
  flush();
}
export function Called() {
  setTimeout(() => {
    plain();
    void runStep(() => {});
  });
  return (
    <Pkg render={() => {}}>
      <button
        onClick={() => {
          flush();
        }}
      >
        x
      </button>
    </Pkg>
  );
}
