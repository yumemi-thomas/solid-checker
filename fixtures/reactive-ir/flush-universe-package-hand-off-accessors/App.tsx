import { flush } from "solid-js";
import { Frame, Pkg, runStep } from "steplib";

// The review of #222's check 3, R1 and R2: an accessor runs where it is read. `steplib` (its source is not read: its manifest names no Solid package) runs what it is handed
// as an action's step. The program names no action, effect or leaf primitive, so the census says the three flush codes cannot arise; a flush the package may run keeps
// its three uncertifiable rows (`flush-in-action-unaccounted`, `flush-in-effect-unaccounted`, `leaf-operation-unaccounted`), never silence, never a violation.

// ---- R1: a project component reads a prop in a step the package runs. The compiler makes the child (or prop) expression a getter of the props object, so the read
// runs it there, whatever is done with the value.
function stepChildCall() {
  flush();
  return "x";
}
function StepReader(props: { children?: unknown }) {
  setTimeout(() => {
    void runStep(() => {
      void props.children;
    });
  });
  return null;
}
setTimeout(() => {
  void (<StepReader>{stepChildCall()}</StepReader>);
});

function StepChildElement() {
  flush();
  return null;
}
function StepElementReader(props: { children?: unknown }) {
  setTimeout(() => {
    void runStep(() => {
      void props.children;
    });
  });
  return null;
}
setTimeout(() => {
  void (<StepElementReader><StepChildElement /></StepElementReader>);
});

function destructuredChild() {
  flush();
  return "x";
}
function DestructuringReader(props: { children?: unknown }) {
  setTimeout(() => {
    void runStep(() => {
      const { children } = props;
      void children;
    });
  });
  return null;
}
setTimeout(() => {
  void (<DestructuringReader>{destructuredChild()}</DestructuringReader>);
});

const directMarker = 0;
function DirectReader(props: { children?: unknown }) {
  setTimeout(() => {
    void runStep(() => {
      void props.children;
    });
  });
  return null;
}
setTimeout(() => {
  void (<DirectReader>{(flush(), directMarker)}</DirectReader>);
});

function renderProp() {
  flush();
  return "x";
}
function RenderReader(props: { x?: unknown }) {
  return <Pkg render={() => {
    void props.x;
  }} />;
}
setTimeout(() => {
  void (<RenderReader x={renderProp()} />);
});

// ---- R2: an object literal's getter runs where a member of the object is read: here through a project function's parameter, in a step the package runs
const getterObject = {
  get g() {
    flush();
    return 1;
  },
};
function readsInStep(p: { g: number }) {
  void runStep(() => {
    void p.g;
  });
}
setTimeout(() => {
  readsInStep(getterObject);
});

// ---- twins, silent: the reads run on stacks no package runs
// a component that reads its children in its own body (it runs where its element is created, the timer's callback)
function twinBodyChild() {
  flush();
  return "x";
}
function BodyReader(props: { children?: unknown }) {
  void props.children;
  return null;
}
setTimeout(() => {
  void (<BodyReader>{twinBodyChild()}</BodyReader>);
});
// a component that reads its children in a timer's callback of its own
function twinTimerChild() {
  flush();
  return "x";
}
function TimerReader(props: { children?: unknown }) {
  setTimeout(() => {
    void props.children;
  });
  return null;
}
setTimeout(() => {
  void (<TimerReader>{twinTimerChild()}</TimerReader>);
});
// a getter read through a parameter in a timer's callback
const twinGetterObject = {
  get g() {
    flush();
    return 1;
  },
};
function readsHere(p: { g: number }) {
  void p.g;
}
setTimeout(() => {
  readsHere(twinGetterObject);
});

// ---- the review of #226. B1: a getter that names its own object and a component that renders itself (each walked the accessor question anew, without end: the
// checker overflowed its stack). Both end now: the getter read in a step keeps its rows; the self-rendering tree never reads `x`, so `treeHelper` never runs.
const selfNamed = {
  x: 1,
  get g(): number {
    flush();
    return selfNamed.x;
  },
};
setTimeout(() => {
  void runStep(() => {
    void selfNamed.g;
  });
});
declare const deeper: boolean;
function treeHelper() {
  flush();
  return "x";
}
function Tree(props: { depth: number; onPick?: () => void; x?: unknown }): JSX.Element {
  const kid = () => <Tree depth={1} onPick={props.onPick} x={treeHelper()} />;
  return deeper ? kid() : null;
}
setTimeout(() => {
  void (<Tree depth={0} />);
});

// B2: a props read written in the component's body, but in a JSX expression of a package's component (which reads it in its step) or in a class field initializer
// (run when the class is made, in a step), runs there
function typeofChild() {
  flush();
  return "x";
}
function TypeofReader(props: { children?: unknown }) {
  return <Frame>{typeof props.children}</Frame>;
}
setTimeout(() => {
  void (<TypeofReader>{typeofChild()}</TypeofReader>);
});
function classFieldChild() {
  flush();
  return "x";
}
function ClassFieldReader(props: { children?: unknown }) {
  class Reads {
    v = props.children === undefined;
  }
  setTimeout(() => {
    void runStep(() => {
      void new Reads();
    });
  });
  return null;
}
setTimeout(() => {
  void (<ClassFieldReader>{classFieldChild()}</ClassFieldReader>);
});

// B3: a getter read through `this` in its own literal's method, in a step
const thisReader = {
  get g(): number {
    flush();
    return 1;
  },
  m() {
    void runStep(() => {
      void this.g;
    });
  },
};
setTimeout(() => {
  thisReader.m();
});
