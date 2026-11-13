// The package hand-off shapes (batch 11, the B3 PR after #213): a flush in a function a package the build does not read as source may run as an action's step, an
// effect's apply or a leaf owner's callback. The census reasons of the flush accounting (no stepped action, no deferring primitive, no leaf scope) read the primitives
// the build's own table names, and a package whose source the build does not read can run what the project hands it under any of them. A route says how the function reaches
// the package:
//
//   ph-<route>-<host>          misuse   the package runs the function as the host: the flush raises the host's code
//   ph-<route>-<host>-never    legal    the same text, the package's function never runs what it is handed
//     routes  literal    `runStep(() => { <site> })`
//             ref        `function cb1() { <site> }`, `runStep(cb1)`: by reference
//             alias      `const cb1 = () => { <site> }; const again1 = cb1;`, `runStep(again1)`: through a copy
//             member     `runStepWith({ run: () => { <site> } })`: a member of an object literal
//             method     `runStepWith({ run() { <site> } })`: a method of one
//             holder     `const opts1 = { run: () => { <site> } };`, `runStepWith(opts1)`: a member of an object a const holds
//             array      `runStepAll([() => { <site> }])`: an element of an array literal
//             jsx        `<StepView render={() => { <site> }} />`: a prop of a component of the package
//             children   `<StepView>{() => { <site> }}</StepView>`: a child expression of one
//             wrap       `function wrap1(run) { runStep(run) }`, `wrap1(() => { <site> })`: through a project function's parameter
//             caller     `function helper1() { <site> }`, `runStep(() => helper1())`: a function the handed callback calls
//             return     `function make1() { return () => { <site> }; }`, `runStep(make1())`: a function a project function returns
//             call       `runStep.call(null, () => { <site> })`: a reflective call that forwards its arguments to the package's function
//           and, the flip of the default (the review of #216: a value counts as handed unless a fact of the program proves where it goes):
//             set        `const set1 = new Set(); set1.add(() => { <site> });`, `set1.forEach((f) => runStep(f))`: kept in a collection, read elsewhere
//             forof      `for (const f of [() => { <site> }]) runStep(f)`: an iterable's element
//             recv       `[() => { <site> }].forEach((f) => runStep(f))`: an array literal that is a method's receiver
//             resolve    `Promise.resolve(() => { <site> }).then((f) => runStep(f))`: a standard call that returns its argument
//             class      `class Job1 { run() { <site> } }`, `runStepWith(new Job1())`: a method of a class whose instance is handed
//             assign     `config.onStep = () => { <site> }`, `runStepConfig()`: an assignment into an object the package exports
//             props      `function Wrap1(props) { return <StepView render={props.render} /> }`, `<Wrap1 render={() => { <site> }} />`: a project component's props
//             getter     `const cb1 = () => { <site> }`, `runStepGet(() => cb1)`: a function the package runs returns it
//             reflect    `function wrap1(run) { runStep(run) }`, `wrap1.call(null, () => { <site> })`: a reflective call whose target is a project function
//             inline     `[1].forEach(cb1)` in a step the package runs: run inline by the standard library on the package's stack
//             param      `call1(cb1)` in a step the package runs, `function call1(g) { g() }`: run inline by a project function's parameter
//             untrack    `untrack(cb1)` in a step the package runs: run inline by a dialect primitive
//             dynamic    `<Dynamic component={StepView} render={() => { <site> }} />`: the component its `component` prop names
//           and the review of #217 (a context places a value only when it carries it: the path from the context's expression to the function is transparent; the value inside
//           a receiver, a template part, an `await` or a `yield` is handed; a function that names `arguments` hands every argument; the call a returning function is an
//           argument of is a position):
//             mapinit    `const r1 = [cb1].map((f) => runStep(f))`: an array literal that is a method's receiver, inside a binding's initializer
//             mapdestr   `const fs1 = [cb1]; const [first1] = fs1; const r1 = [first1].map((f) => runStep(f))`: an element read back through a destructuring
//             mapparam   `function w1(g) { const r1 = [g].map((f) => runStep(f)) }`, `w1(cb1)`: the same behind a project function's parameter
//             mapflat    `const r1 = [[cb1]].flat().map((f) => runStep(f))`: a nested literal
//             maptemplate  a template literal's part: `${[cb1].map((f) => runStep(f)).length}`
//             maplogical `{ a: cb1 }.a.name.length > 0 && [cb1].some((f) => { runStep(f); return true; })`: an object literal read through a member, a logical expression
//             mapawait   `await [cb1].map((f) => runStep(f))[0]`: an `await` operand
//             mapassign  `kept1 = [cb1].map((f) => runStep(f))`: an assignment's value
//             maparg     `use1([cb1].map((f) => runStep(f)))`: an argument of another call
//             yieldof    `function* gen1(): Generator<() => void, void, undefined> { const x = yield cb1; }`, `for (const f of gen1()) runStep(f)`: a `yield` operand
//             nbmap      `function step1() { const rs = [cb1].map((f) => f()); }`, `runStep(step1)`: the literal's elements are called in the step the package runs
//             nbobj      `function step1() { const r = { run: cb1 }.run(); }`, `runStep(step1)`: a member of an object literal is called in the step
//             argsindex  `function w1(f) { runStep(arguments[0]); }`, `w1(cb1)`: a function that reads its argument through `arguments`
//             argsapply  `function w3(...fs) { runStep(fs[0]); } function w4(f) { w3.apply(null, arguments); }`, `w4(cb1)`: `arguments` handed whole to a reflective call
//             thenreturn `Promise.resolve().then(() => cb1).then((f) => runStep(f))`: a function written as an argument returns it to the call
//             refprop    `function step1() { void (<div ref={cb1} />); }`, `runStep(step1)`: an intrinsic element's `ref` callback is applied where the element is created
//             dynref     `function step1() { void (<Dynamic component="div" ref={cb1} />); }`, `runStep(step1)`: the same through `Dynamic` with a string tag (the step host only: `Dynamic`
//                        creates a memo, which an effect's apply and a leaf owner's callback refuse before the `ref` is applied)
//           and the review of #222 (an assignment expression's own value is the value it writes; an intrinsic element applies its `ref`, its children and its spreads where
//           it is created):
//             chainconst `let last1: unknown; const first1 = (last1 = cb1); runStep(first1)`: the value of an assignment expression, a binding's initializer
//             chainassign `first1 = last1 = cb1; runStep(first1)`: a chained assignment
//             chainrecv  `const r1 = [(last1 = cb1)].map((f) => runStep(f))`: an assignment expression that is an element of a receiver
//             spreadref  `function step1() { void (<div {...{ ref: cb1 }} />); }`, `runStep(step1)`: a `ref` reaching an intrinsic element through a spread (step host only: the
//                        spread creates an effect, which an effect's apply and a leaf owner's callback refuse)
//             spreadvar  `const p1 = { ref: cb1 }; function step1() { void (<div {...p1} />); }`: the same through a binding, whose keys are not read (step host only)
//             childfn    `function step1() { void (<div>{cb1 as never}</div>); }`, `runStep(step1)`: a function child of an intrinsic element, called as an accessor where the
//                        element is created (step host only: the insertion creates an effect)
//             framechild `const Own1 = ((_props) => { <site> }) as unknown as …; <StepView>{(<Own1>x</Own1>) as never}</StepView>`: a component written as a child of the
//                        package's component, created when that component reads its children (both tag names; step host only: Chrome never runs the component
//                        when the view reads its children in an effect's apply or a tracked compute)
//             framebare  the same with `<Own1 />`
//           and the re-review of #222 (N1: an expression in an unread component's children or props runs when the component reads it, so what it calls or creates is
//           handed; step host only, as framechild):
//             framemap   `<StepView>{[1].map(() => <Own1 />) as never}</StepView>`: an element in a callback of an inline call
//             framecall  `function h1() { <site> }`, `<StepView>{h1() as never}</StepView>`: a call
//             framerender `function m1() { <site> }`, `<StepView render={m1() as never} />`: a call in a prop
//             frameiife  `<StepView>{(() => <Own1 />)() as never}</StepView>`: an immediately called function
//             framemapref `<StepView>{[1].map(() => <div ref={cb1} />) as never}</StepView>`: an intrinsic `ref` in a callback of an inline call
//             framevoid, frametypeof  `{void (<Own1 />)}`, `{typeof (<Own1 />) as never}`: an element created under an operand nothing takes
//           A flush written in the child expression itself (`{(flush(), null)}`) is not a route: a site is a function's statements (fixture and `pkghand/frame-flush-step`).
//           and the review of #222's check 3 (an accessor runs where it is read: a component's prop expression is a getter of its props object, an object literal's
//           getter runs at a member read):
//             propsread  `function h1() { <site> }`, `function Wrap1(props) { setTimeout(() => { runStep(() => { void props.children; }); }); }`, `<Wrap1>{h1()}</Wrap1>`
//             propsrender `function Wrap1(props) { return <StepView render={() => { void props.x; }} />; }`, `<Wrap1 x={h1()} />`
//             getterparam `const cb1 = () => { <site> }`, `const o1 = { get g() { cb1(); return 1; } }`, `function w1(p) { runStep(() => { void p.g; }); }`, `w1(o1)`
//           and the review of #226 (B1: a getter that names its own object, a component that renders itself; B2: a props read in a JSX expression of the package's
//           component or in a class field initializer; B3: a getter read through `this`):
//             selfgetter `const o1 = { x: 1, get g() { cb1(); return o1.x; } }`, `runStep(() => { void o1.g; })`
//             treerec    `function Tree1(props) { const kid = () => <Tree1 depth={1} pick={props.pick} x={h1()} />; setTimeout(() => runStep(() => { void props.x; })); … }`
//             propsjsx   `function Wrap1(props) { return <StepView>{typeof props.children as never}</StepView>; }`, `<Wrap1>{h1()}</Wrap1>`
//             propsclass `function Wrap1(props) { class K { v = props.children === undefined; } setTimeout(() => runStep(() => { void new K(); })); }`
//             getterthis `const o1 = { get g() { cb1(); return 1; }, m() { runStep(() => { void this.g; }); } }`, `o1.m()`
//           and the re-check of #226 (N1: a method call on an object of accessors binds `this` to it; N2: one callback returns the flushing function or the object):
//             getterouter `function m1(this) { runStep(() => { void this.g; }); }`, `const o1 = { get g() { cb1(); return 1; }, m: m1 }`, `o1.m()`: a method written outside the literal
//             getterassign the same, `m() {}` in the literal and `o1.m = m1` after
//             propsmethod `function Wrap1(props) { setTimeout(() => { props.pick(); }); }`, `pick={m1}`, `m1` reading `this.children` in the step: a prop that is a method
//             getterpick `const pick1 = () => (flag1 ? helper1 : o1)`, `const got1 = [0].map(pick1)`, `runStep(() => { void got1[0].g; })`
//           and the re-check 2 of #226 (`super.x` reads the prototype's member with the object as `this`):
//             gettersuper `const base1 = { get x() { return this.g; } }`, `const o1 = { get g() { cb1(); return 1; }, get h() { runStep(() => { void super.x; }); return 1; } }`,
//                         `o1.__proto__ = base1`, `void o1.h` in a timer: an accessor that holds `super`, no member call
//             methodsuper the same with `runIt() { runStep(() => { void super.x; }); }` and `o1.runIt()`: P570
//           and the check of #227 (an object's members are read through `this` and `super`, which no reference of the object shows; G1 and its neighbours):
//             protogetter  `const base1 = { get g() { cb1(); return 1; } }`, `const o1 = { __proto__: base1, get h() { runStep(() => { void super.g; }); return 1; } }`, `void o1.h` in a timer
//             protogetterthis the same read through `this.g`
//             protoassign the prototype written after the literal: `o1.__proto__ = base1`
//             protomethod `const base1 = { m() { cb1(); } }`, the derived accessor runs `this.m()` in the step
//             ownmethod   `const o1 = { m() { cb1(); }, run() { runStep(() => { this.m(); }); } }`, `o1.run()` in a timer: no prototype, no call edge reaches `m`
//             ownvalue    the same with `runStep(this.m)`
//             foreignthis `function g1(this: any) { runStep(() => { this.m(); }); }`, `const o1 = { m() { cb1(); }, g: g1 }`, `o1.g()`: a method written outside the literal that names `this`
//           and the check of #229 (B1: the method that names `this` reaches the literal in a way no reference resolves, and the call is of a member nothing resolves to one function):
//             foreignparam `function mk1(f: any) { return { m() { cb1(); }, g: f }; }`, `const o1 = mk1(function (this: any) { runStep(() => { this.m(); }); })`, `o1.g()`: through a parameter
//             foreigncall `g: make1()`, `function make1() { return function (this: any) { … }; }`: through a call's result
//             foreignalias `const { a: h1 } = src1; const o1 = { m() { cb1(); }, h1 }`, `o1.h1()`: an alias a destructuring makes
//             foreignarray `const fns1 = [function (this: any) { … }]`, `g: fns1[0]`: an array's element
//           and the review of #229 (B2: a member reaches the receiver by no placement in the literal: the object that holds it is no reference a read of it follows; S2):
//             lateassign  `const o1 = { get h() { runStep(() => { (this as any).m1(); }); return 1; }, m1: undefined as any }`, `o1.m1 = function () { cb1(); }` after the literal
//             latesetter  the same through a setter: `set h(value) { … }`, `o1.h = 1` in a timer
//             latenested  `o1.sub1 = { m() { cb1(); } }`, the getter runs `this.sub1.m()`
//             lateput     the member written by a project function: `function put1(target: any) { target.m1 = function () { cb1(); }; }`, `put1(o1)`
//             protoreceiver the receiver on the prototype, the flushing member on the derived literal: `const o1 = { __proto__: base1, m1() { cb1(); } }`
//             protocreate `const o1: any = Object.create(base1)`, `o1.m1 = function () { cb1(); }`
//             protoclass  `class K1 { m1: any; get h() { runStep(() => { this.m1(); }); return 1; } }`, `const o1 = new K1()`, `o1.m1 = function () { cb1(); }`
//             protokey    a prototype written through a computed key the program does not spell: `const k1 = "__pro" + "to__"`, `o1[k1] = base1`
//             lateself    the receiver kept in a local (`const self1 = this`) and read in the step: `self1.m1()`
//             latedestr   `const { m1 } = this as any` in the step
//             latecomputed `(this as any)[key1]()`, a key the program does not name
//           and the neighbours the probing of #229's B2 found (no receiver in them: a function stored through a parameter's member is stored into the object the callers pass,
//           and the code that reads that object reads it by the caller's name):
//             putparam    `function put1(target: { m1(): void }) { target.m1 = function () { cb1(); }; }`, `const o1 = { m1(): void {} }`, `put1(o1)`, `runStep(() => { o1.m1(); })`
//             putparamany the same with `target: any` and `const o1: any = {}`
//             putchain    the argument is the parameter of an outer function: `function putOuter1(holder: …) { put1(holder); }`, `putOuter1(o1)`
//           These routes are written at module scope (`top`), beside the component, not in its body: a function written in the body of an exported component is run by a package
//           whenever the program's boundary is open (the fuzz programs' is), so a cell there is handed by the component and says nothing of the position the route is about.
//   hosts   step (the package runs it in an action's step: FLUSH_IN_ACTION), apply (an effect's apply: FLUSH_IN_EFFECT_CALLBACK), leaf (a leaf owner's callback:
//           FLUSH_IN_TRACKED_QUEUE_CALLBACK)
//
// The package `fz-step` is written here, a manifest with no dependency on a Solid package (so that the checker does not read its source, which is the premise of the shape;
// `peerDependencies` naming solid-js would make it a dependency read as source: dependency_sources.rs), and exports one function per host and route kind, and a `skip*`
// that ignores what it is handed, the twin's. Chrome (rc.21, the real oracle) judges each cell: what it reports is what the checker is compared with. Recall contexts, the
// direct binding bare (`phShape`), not crossed by the census, out of api-fire's plan.
import { contain } from "./model.mjs";

export const PH_PACKAGE = "fz-step";
export const PH_ROUTES = ["literal", "ref", "alias", "member", "method", "holder", "array", "jsx", "children", "wrap", "caller", "return", "call", "set", "forof", "recv", "resolve", "class", "assign", "props", "getter", "reflect", "inline", "param", "untrack", "dynamic", "mapinit", "mapdestr", "mapparam", "mapflat", "maptemplate", "maplogical", "mapawait", "mapassign", "maparg", "yieldof", "nbmap", "nbobj", "argsindex", "argsapply", "thenreturn", "thenconst", "thenlet", "thenfn", "thenmember", "refprop", "dynref", "chainconst", "chainassign", "chainrecv", "spreadref", "spreadvar", "childfn", "framechild", "framebare", "framemap", "framecall", "framerender", "frameiife", "framemapref", "framevoid", "frametypeof", "propsread", "propsrender", "getterparam", "selfgetter", "treerec", "propsjsx", "propsclass", "getterthis", "getterouter", "getterassign", "propsmethod", "getterpick", "gettersuper", "methodsuper", "protogetter", "protogetterthis", "protoassign", "protomethod", "ownmethod", "ownvalue", "foreignthis", "foreignparam", "foreigncall", "foreignalias", "foreignarray", "lateassign", "latesetter", "latenested", "lateput", "protoreceiver", "protocreate", "protoclass", "protokey", "lateself", "latedestr", "latecomputed", "putparam", "putparamany", "putchain"];
export const PH_HOSTS = ["step", "apply", "leaf"];
/** The hosts each code is judged in: the code the host's callback reports for a flush in it. */
export const PH_CODES = {
  FLUSH_IN_EFFECT_CALLBACK: ["apply"],
  FLUSH_IN_ACTION: ["step"],
  FLUSH_IN_TRACKED_QUEUE_CALLBACK: ["leaf"],
};
/** `{ [context]: "misuse" | "legal" }`. */
/** The routes that exist on some hosts only: `Dynamic` creates a memo, and a spread or an inserted child an effect, which an effect's apply and a leaf owner's callback refuse (PRIMITIVE_IN_EFFECT_CALLBACK, before the callback is applied); the frame routes' component, read from the view's children there, never runs in Chrome. */
const ROUTE_HOSTS = { dynref: ["step"], spreadref: ["step"], spreadvar: ["step"], childfn: ["step"], framechild: ["step"], framebare: ["step"], framemap: ["step"], framecall: ["step"], framerender: ["step"], frameiife: ["step"], framemapref: ["step"], framevoid: ["step"], frametypeof: ["step"] };
export const PH_CONTEXTS = Object.fromEntries(PH_ROUTES.flatMap((route) => PH_HOSTS.filter((host) => (ROUTE_HOSTS[route] ?? PH_HOSTS).includes(host)).flatMap((host) => [[`ph-${route}-${host}`, "misuse"], [`ph-${route}-${host}-never`, "legal"]])));
export const PH_CONTEXT_NAMES = Object.keys(PH_CONTEXTS);

const NAME = new RegExp(`^ph-(${PH_ROUTES.join("|")})-(${PH_HOSTS.join("|")})(-never)?$`);
/** `{ route, host, never }` of a `ph-*` context name, or null. */
export const phParts = (context) => {
  const match = NAME.exec(context);
  return match ? { route: match[1], host: match[2], never: Boolean(match[3]) } : null;
};

/** The base shapes a `ph-*` cell takes: the direct binding, no layer. */
export function phShape(context, shape) {
  return !phParts(context) || (shape.binding === "direct" && (shape.layer ?? "none") === "none");
}

const UPPER = { step: "Step", apply: "Apply", leaf: "Leaf" };
/** The package function a route calls for a host: its name, the import it needs. */
const callee = (route, host, never) => {
  if (["jsx", "children", "props", "dynamic", "framechild", "framebare", "framemap", "framecall", "framerender", "frameiife", "framemapref", "framevoid", "frametypeof", "propsrender", "propsjsx"].includes(route)) return never ? "SkipView" : `${UPPER[host]}View`;
  if (route === "getter") return never ? "skipRunGet" : `run${UPPER[host]}Get`;
  if (route === "assign") return never ? "skipRunConfig" : `run${UPPER[host]}Config`;
  const kind = ["member", "method", "holder", "class"].includes(route) ? "With" : route === "array" ? "All" : "";
  return never ? `skipRun${kind}` : `run${UPPER[host]}${kind}`;
};

/** The frame text of a route: `head` precedes the body (the callback's or the function's block), `close` follows it. */
const ROUTE_FRAMES = {
  literal: (f) => ({ head: `setTimeout(() => {\n  ${f}(() => `, close: `);\n});` }),
  ref: (f, n) => {
    const cb = n("cb");
    return { head: `function ${cb}() `, close: `\nsetTimeout(() => {\n  ${f}(${cb});\n});` };
  },
  alias: (f, n) => {
    const cb = n("cb");
    const again = n("again");
    return { head: `const ${cb} = () => `, close: `;\nconst ${again} = ${cb};\nsetTimeout(() => {\n  ${f}(${again});\n});` };
  },
  member: (f) => ({ head: `setTimeout(() => {\n  ${f}({ run: () => `, close: ` });\n});` }),
  method: (f) => ({ head: `setTimeout(() => {\n  ${f}({ run() `, close: ` });\n});` }),
  holder: (f, n) => {
    const opts = n("opts");
    return { head: `const ${opts} = { run: () => `, close: ` };\nsetTimeout(() => {\n  ${f}(${opts});\n});` };
  },
  array: (f) => ({ head: `setTimeout(() => {\n  ${f}([() => `, close: `]);\n});` }),
  jsx: (f) => ({ head: `setTimeout(() => {\n  document.body.append((<${f} render={() => `, close: `} />) as unknown as Node);\n});` }),
  children: (f) => ({ head: `setTimeout(() => {\n  document.body.append((<${f}>{() => `, close: `}</${f}>) as unknown as Node);\n});` }),
  wrap: (f, n) => {
    const wrap = n("wrap");
    return { head: `function ${wrap}(run: () => void) {\n  ${f}(run);\n}\nsetTimeout(() => {\n  ${wrap}(() => `, close: `);\n});` };
  },
  caller: (f, n) => {
    const helper = n("helper");
    return { head: `function ${helper}() `, close: `\nsetTimeout(() => {\n  ${f}(() => ${helper}());\n});` };
  },
  return: (f, n) => {
    const make = n("make");
    return { head: `function ${make}() {\n  return () => `, close: `;\n}\nsetTimeout(() => {\n  ${f}(${make}());\n});` };
  },
  call: (f) => ({ head: `setTimeout(() => {\n  ${f}.call(null, () => `, close: `);\n});` }),
  set: (f, n) => {
    const kept = n("kept");
    const each = n("each");
    return { head: `const ${kept} = new Set<() => void>();\n${kept}.add(() => `, close: `);\nsetTimeout(() => {\n  ${kept}.forEach((${each}) => {\n    ${f}(${each});\n  });\n});` };
  },
  forof: (f, n) => {
    const each = n("each");
    return { head: `setTimeout(() => {\n  for (const ${each} of [() => `, close: `]) {\n    ${f}(${each});\n  }\n});` };
  },
  recv: (f, n) => {
    const each = n("each");
    return { head: `setTimeout(() => {\n  [() => `, close: `].forEach((${each}) => {\n    ${f}(${each});\n  });\n});` };
  },
  resolve: (f, n) => {
    const each = n("each");
    return { head: `setTimeout(() => {\n  void Promise.resolve(() => `, close: `).then((${each}) => {\n    ${f}(${each});\n  });\n});` };
  },
  class: (f, n) => {
    const job = n("Job");
    return { head: `class ${job} {\n  run() `, close: `\n}\nsetTimeout(() => {\n  ${f}(new ${job}());\n});` };
  },
  assign: (f) => ({ head: `setTimeout(() => {\n  config.onStep = () => `, close: `;\n  ${f}();\n});` }),
  props: (f, n) => {
    const wrap = n("Wrap");
    return { head: `function ${wrap}(props: { render: () => void }) {\n  return <${f} render={props.render} />;\n}\nsetTimeout(() => {\n  document.body.append((<${wrap} render={() => `, close: `} />) as unknown as Node);\n});` };
  },
  getter: (f, n) => {
    const cb = n("cb");
    return { head: `const ${cb} = () => `, close: `;\nsetTimeout(() => {\n  ${f}(() => ${cb});\n});` };
  },
  reflect: (f, n) => {
    const wrap = n("wrap");
    return { head: `function ${wrap}(run: () => void) {\n  ${f}(run);\n}\nsetTimeout(() => {\n  ${wrap}.call(null, () => `, close: `);\n});` };
  },
  inline: (f, n) => {
    const cb = n("cb");
    const step = n("step");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${step}() {\n  [1].forEach(${cb});\n}\nsetTimeout(() => {\n  ${f}(${step});\n});` };
  },
  param: (f, n) => {
    const cb = n("cb");
    const call = n("call");
    const step = n("step");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${call}(g: () => void) {\n  g();\n}\nfunction ${step}() {\n  ${call}(${cb});\n}\nsetTimeout(() => {\n  ${f}(${step});\n});` };
  },
  untrack: (f, n) => {
    const cb = n("cb");
    const step = n("step");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${step}() {\n  untrack(${cb});\n}\nsetTimeout(() => {\n  ${f}(${step});\n});` };
  },
  dynamic: (f) => ({ head: `setTimeout(() => {\n  document.body.append((<Dynamic component={${f}} render={() => `, close: `} />) as unknown as Node);\n});` }),
  mapinit: (f, n) => {
    const cb = n("cb");
    const rs = n("rs");
    return { head: `const ${cb} = () => `, close: `;\nsetTimeout(() => {\n  const ${rs} = [${cb}].map((each) => ${f}(each));\n});` };
  },
  mapdestr: (f, n) => {
    const cb = n("cb");
    const fs = n("fs");
    const first = n("first");
    const rs = n("rs");
    return { head: `const ${cb} = () => `, close: `;\nconst ${fs} = [${cb}];\nconst [${first}] = ${fs};\nsetTimeout(() => {\n  const ${rs} = [${first}].map((each) => ${f}(each));\n});` };
  },
  mapparam: (f, n) => {
    const cb = n("cb");
    const wrap = n("wrap");
    const rs = n("rs");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${wrap}(g: () => void) {\n  const ${rs} = [g].map((each) => ${f}(each));\n}\nsetTimeout(() => {\n  ${wrap}(${cb});\n});` };
  },
  mapflat: (f, n) => {
    const cb = n("cb");
    const rs = n("rs");
    return { head: `const ${cb} = () => `, close: `;\nsetTimeout(() => {\n  const ${rs} = [[${cb}]].flat().map((each) => ${f}(each));\n});` };
  },
  maptemplate: (f, n) => {
    const cb = n("cb");
    const text = n("text");
    return { head: `const ${cb} = () => `, close: `;\nsetTimeout(() => {\n  const ${text} = \`\${[${cb}].map((each) => ${f}(each)).length}\`;\n});` };
  },
  maplogical: (f, n) => {
    const cb = n("cb");
    const ok = n("ok");
    return { head: `const ${cb} = () => `, close: `;\nsetTimeout(() => {\n  const ${ok} = { a: ${cb} }.a.name.length > 0 && [${cb}].some((each) => {\n    ${f}(each);\n    return true;\n  });\n});` };
  },
  mapawait: (f, n) => {
    const cb = n("cb");
    return { head: `const ${cb} = () => `, close: `;\nsetTimeout(async () => {\n  await [${cb}].map((each) => ${f}(each))[0];\n});` };
  },
  mapassign: (f, n) => {
    const cb = n("cb");
    const kept = n("kept");
    return { head: `const ${cb} = () => `, close: `;\nlet ${kept}: unknown;\nsetTimeout(() => {\n  ${kept} = [${cb}].map((each) => ${f}(each));\n});` };
  },
  maparg: (f, n) => {
    const cb = n("cb");
    const use = n("use");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${use}(value: unknown) {\n  return value;\n}\nsetTimeout(() => {\n  ${use}([${cb}].map((each) => ${f}(each)));\n});` };
  },
  yieldof: (f, n) => {
    const cb = n("cb");
    const gen = n("gen");
    const x = n("x");
    return { head: `const ${cb} = () => `, close: `;\nfunction* ${gen}(): Generator<() => void, void, undefined> {\n  const ${x} = yield ${cb};\n}\nsetTimeout(() => {\n  for (const each of ${gen}()) {\n    ${f}(each);\n  }\n});` };
  },
  nbmap: (f, n) => {
    const cb = n("cb");
    const step = n("step");
    const rs = n("rs");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${step}() {\n  const ${rs} = [${cb}].map((each) => each());\n}\nsetTimeout(() => {\n  ${f}(${step});\n});` };
  },
  nbobj: (f, n) => {
    const cb = n("cb");
    const step = n("step");
    const r = n("r");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${step}() {\n  const ${r} = { run: ${cb} }.run();\n}\nsetTimeout(() => {\n  ${f}(${step});\n});` };
  },
  argsindex: (f, n) => {
    const cb = n("cb");
    const wrap = n("wrap");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${wrap}(g: () => void) {\n  void g;\n  ${f}(arguments[0]);\n}\nsetTimeout(() => {\n  ${wrap}(${cb});\n});` };
  },
  argsapply: (f, n) => {
    const cb = n("cb");
    const rest = n("rest");
    const wrap = n("wrap");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${rest}(...fs: Array<() => void>) {\n  ${f}(fs[0]);\n}\nfunction ${wrap}(g: () => void) {\n  void g;\n  ${rest}.apply(null, arguments as unknown as Array<() => void>);\n}\nsetTimeout(() => {\n  ${wrap}(${cb});\n});` };
  },
  thenreturn: (f, n) => {
    const cb = n("cb");
    return { head: `const ${cb} = () => `, close: `;\nsetTimeout(() => {\n  void Promise.resolve()\n    .then(() => ${cb})\n    .then((each) => {\n      ${f}(each);\n    });\n});` };
  },
  dynref: (f, n) => {
    const cb = n("cb");
    const step = n("step");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${step}() {\n  void (<Dynamic component="div" ref={${cb}} />);\n}\nsetTimeout(() => {\n  ${f}(${step});\n});` };
  },
  thenconst: (f, n) => {
    const cb = n("cb");
    const give = n("give");
    return { head: `const ${cb} = () => `, close: `;\nconst ${give} = () => ${cb};\nsetTimeout(() => {\n  void Promise.resolve().then(${give}).then((each) => ${f}(each));\n});` };
  },
  thenlet: (f, n) => {
    const cb = n("cb");
    const give = n("give");
    return { head: `const ${cb} = () => `, close: `;\nlet ${give} = () => ${cb};\nsetTimeout(() => {\n  void Promise.resolve().then(${give}).then((each) => ${f}(each));\n});` };
  },
  thenfn: (f, n) => {
    const cb = n("cb");
    const give = n("give");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${give}() { return ${cb}; }\nsetTimeout(() => {\n  void Promise.resolve().then(${give}).then((each) => ${f}(each));\n});` };
  },
  thenmember: (f, n) => {
    const cb = n("cb");
    const give = n("give");
    return { head: `const ${cb} = () => `, close: `;\nconst ${give} = { give: () => ${cb} };\nsetTimeout(() => {\n  void Promise.resolve().then(${give}.give).then((each) => ${f}(each));\n});` };
  },
  refprop: (f, n) => {
    const cb = n("cb");
    const step = n("step");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${step}() {\n  void (<div ref={${cb}} />);\n}\nsetTimeout(() => {\n  ${f}(${step});\n});` };
  },
  chainconst: (f, n) => {
    const cb = n("cb");
    const last = n("last");
    const first = n("first");
    return { head: `const ${cb} = () => `, close: `;\nlet ${last}: unknown;\nsetTimeout(() => {\n  const ${first} = (${last} = ${cb});\n  ${f}(${first});\n});` };
  },
  chainassign: (f, n) => {
    const cb = n("cb");
    const last = n("last");
    const first = n("first");
    return { head: `const ${cb} = () => `, close: `;\nlet ${first}: () => void = () => {};\nlet ${last}: unknown;\nsetTimeout(() => {\n  ${first} = ${last} = ${cb};\n  ${f}(${first});\n});` };
  },
  chainrecv: (f, n) => {
    const cb = n("cb");
    const last = n("last");
    const rs = n("rs");
    return { head: `const ${cb} = () => `, close: `;\nlet ${last}: unknown;\nsetTimeout(() => {\n  const ${rs} = [(${last} = ${cb})].map((each) => ${f}(each));\n});` };
  },
  spreadref: (f, n) => {
    const cb = n("cb");
    const step = n("step");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${step}() {\n  void (<div {...{ ref: ${cb} }} />);\n}\nsetTimeout(() => {\n  ${f}(${step});\n});` };
  },
  spreadvar: (f, n) => {
    const cb = n("cb");
    const props = n("props");
    const step = n("step");
    return { head: `const ${cb} = () => `, close: `;\nconst ${props} = { ref: ${cb} };\nfunction ${step}() {\n  void (<div {...${props}} />);\n}\nsetTimeout(() => {\n  ${f}(${step});\n});` };
  },
  childfn: (f, n) => {
    const cb = n("cb");
    const step = n("step");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${step}() {\n  void (<div>{${cb} as never}</div>);\n}\nsetTimeout(() => {\n  ${f}(${step});\n});` };
  },
  framechild: (f, n) => {
    const own = n("Own");
    return { head: `const ${own} = ((_props: { children?: unknown }) => `, close: `) as unknown as (props: { children?: unknown }) => null;\nsetTimeout(() => {\n  document.body.append((<${f}>{(<${own}>x</${own}>) as never}</${f}>) as unknown as Node);\n});` };
  },
  framebare: (f, n) => {
    const own = n("Own");
    return { head: `const ${own} = ((_props: { children?: unknown }) => `, close: `) as unknown as (props: { children?: unknown }) => null;\nsetTimeout(() => {\n  document.body.append((<${f}>{(<${own} />) as never}</${f}>) as unknown as Node);\n});` };
  },
  framemap: (f, n) => {
    const own = n("Own");
    return { head: `const ${own} = ((_props: { children?: unknown }) => `, close: `) as unknown as (props: { children?: unknown }) => null;\nsetTimeout(() => {\n  document.body.append((<${f}>{[1].map(() => <${own} />) as never}</${f}>) as unknown as Node);\n});` };
  },
  framecall: (f, n) => {
    const helper = n("helper");
    return { head: `function ${helper}() `, close: `\nsetTimeout(() => {\n  document.body.append((<${f}>{${helper}() as never}</${f}>) as unknown as Node);\n});` };
  },
  framerender: (f, n) => {
    const make = n("make");
    return { head: `function ${make}() `, close: `\nsetTimeout(() => {\n  document.body.append((<${f} render={${make}() as never} />) as unknown as Node);\n});` };
  },
  frameiife: (f, n) => {
    const own = n("Own");
    return { head: `const ${own} = ((_props: { children?: unknown }) => `, close: `) as unknown as (props: { children?: unknown }) => null;\nsetTimeout(() => {\n  document.body.append((<${f}>{(() => <${own} />)() as never}</${f}>) as unknown as Node);\n});` };
  },
  framemapref: (f, n) => {
    const cb = n("cb");
    return { head: `const ${cb} = () => `, close: `;\nsetTimeout(() => {\n  document.body.append((<${f}>{[1].map(() => <div ref={${cb}} />) as never}</${f}>) as unknown as Node);\n});` };
  },
  framevoid: (f, n) => {
    const own = n("Own");
    return { head: `const ${own} = ((_props: { children?: unknown }) => `, close: `) as unknown as (props: { children?: unknown }) => null;\nsetTimeout(() => {\n  document.body.append((<${f}>{void (<${own} />)}</${f}>) as unknown as Node);\n});` };
  },
  frametypeof: (f, n) => {
    const own = n("Own");
    return { head: `const ${own} = ((_props: { children?: unknown }) => `, close: `) as unknown as (props: { children?: unknown }) => null;\nsetTimeout(() => {\n  document.body.append((<${f}>{typeof (<${own} />) as never}</${f}>) as unknown as Node);\n});` };
  },
  propsread: (f, n) => {
    const helper = n("helper");
    const wrap = n("Wrap");
    return { head: `function ${helper}() `, close: `\nfunction ${wrap}(props: { children?: unknown }) {\n  setTimeout(() => {\n    ${f}(() => {\n      void props.children;\n    });\n  });\n  return null;\n}\nsetTimeout(() => {\n  document.body.append((<${wrap}>{${helper}()}</${wrap}>) as unknown as Node);\n});` };
  },
  propsrender: (f, n) => {
    const helper = n("helper");
    const wrap = n("Wrap");
    return { head: `function ${helper}() `, close: `\nfunction ${wrap}(props: { x?: unknown }) {\n  return <${f} render={() => {\n    void props.x;\n  }} />;\n}\nsetTimeout(() => {\n  document.body.append((<${wrap} x={${helper}()} />) as unknown as Node);\n});` };
  },
  getterparam: (f, n) => {
    const cb = n("cb");
    const object = n("o");
    const reader = n("w");
    return { head: `const ${cb} = () => `, close: `;\nconst ${object} = {\n  get g() {\n    ${cb}();\n    return 1;\n  },\n};\nfunction ${reader}(p: { g: number }) {\n  ${f}(() => {\n    void p.g;\n  });\n}\nsetTimeout(() => {\n  ${reader}(${object});\n});` };
  },
  selfgetter: (f, n) => {
    const cb = n("cb");
    const object = n("o");
    return { head: `const ${cb} = () => `, close: `;\nconst ${object} = {\n  x: 1,\n  get g(): number {\n    ${cb}();\n    return ${object}.x;\n  },\n};\nsetTimeout(() => {\n  ${f}(() => {\n    void ${object}.g;\n  });\n});` };
  },
  treerec: (f, n) => {
    const helper = n("helper");
    const tree = n("Tree");
    return { head: `function ${helper}() `, close: `\nfunction ${tree}(props: { depth: number; pick?: () => void; x?: unknown }): any {\n  const kid = () => <${tree} depth={1} pick={props.pick} x={${helper}()} />;\n  setTimeout(() => {\n    ${f}(() => {\n      void props.x;\n    });\n  });\n  return props.depth > 0 ? null : kid();\n}\nsetTimeout(() => {\n  document.body.append((<${tree} depth={0} />) as unknown as Node);\n});` };
  },
  propsjsx: (f, n) => {
    const helper = n("helper");
    const wrap = n("Wrap");
    return { head: `function ${helper}() `, close: `\nfunction ${wrap}(props: { children?: unknown }) {\n  return <${f}>{typeof props.children as never}</${f}>;\n}\nsetTimeout(() => {\n  document.body.append((<${wrap}>{${helper}()}</${wrap}>) as unknown as Node);\n});` };
  },
  propsclass: (f, n) => {
    const helper = n("helper");
    const wrap = n("Wrap");
    return { head: `function ${helper}() `, close: `\nfunction ${wrap}(props: { children?: unknown }) {\n  class Reads {\n    v = props.children === undefined;\n  }\n  setTimeout(() => {\n    ${f}(() => {\n      void new Reads();\n    });\n  });\n  return null;\n}\nsetTimeout(() => {\n  document.body.append((<${wrap}>{${helper}()}</${wrap}>) as unknown as Node);\n});` };
  },
  getterthis: (f, n) => {
    const cb = n("cb");
    const object = n("o");
    return { head: `const ${cb} = () => `, close: `;\nconst ${object} = {\n  get g(): number {\n    ${cb}();\n    return 1;\n  },\n  m() {\n    ${f}(() => {\n      void this.g;\n    });\n  },\n};\nsetTimeout(() => {\n  ${object}.m();\n});` };
  },
  getterouter: (f, n) => {
    const cb = n("cb");
    const object = n("o");
    const method = n("m");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${method}(this: { g: number }) {\n  ${f}(() => {\n    void this.g;\n  });\n}\nconst ${object} = {\n  get g(): number {\n    ${cb}();\n    return 1;\n  },\n  m: ${method},\n};\nsetTimeout(() => {\n  ${object}.m();\n});` };
  },
  getterassign: (f, n) => {
    const cb = n("cb");
    const object = n("o");
    const method = n("m");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${method}(this: { g: number }) {\n  ${f}(() => {\n    void this.g;\n  });\n}\nconst ${object} = {\n  get g(): number {\n    ${cb}();\n    return 1;\n  },\n  m() {},\n};\n${object}.m = ${method};\nsetTimeout(() => {\n  ${object}.m();\n});` };
  },
  propsmethod: (f, n) => {
    const helper = n("helper");
    const method = n("m");
    const wrap = n("Wrap");
    return { head: `function ${helper}() `, close: `\nfunction ${method}(this: { children?: unknown }) {\n  ${f}(() => {\n    void this.children;\n  });\n}\nfunction ${wrap}(props: { children?: unknown; pick: (this: { children?: unknown }) => void }) {\n  setTimeout(() => {\n    props.pick();\n  });\n  return null;\n}\nsetTimeout(() => {\n  document.body.append((<${wrap} pick={${method}}>{${helper}()}</${wrap}>) as unknown as Node);\n});` };
  },
  getterpick: (f, n) => {
    const helper = n("helper");
    const object = n("o");
    const pick = n("pick");
    const got = n("got");
    const flag = n("flag");
    return { head: `const ${flag} = Math.random() > 2;\nfunction ${helper}() `, close: `\nconst ${object} = {\n  get g(): number {\n    ${helper}();\n    return 1;\n  },\n};\nconst ${pick} = (): any => (${flag} ? ${helper} : ${object});\nconst ${got} = [0].map(${pick});\nsetTimeout(() => {\n  ${f}(() => {\n    void ${got}[0].g;\n  });\n});` };
  },
  gettersuper: (f, n) => {
    const cb = n("cb");
    const base = n("base");
    const object = n("o");
    return { head: `const ${cb} = () => `, close: `;\nconst ${base} = {\n  get x(): number {\n    return (this as any).g;\n  },\n};\nconst ${object} = {\n  get g(): number {\n    ${cb}();\n    return 1;\n  },\n  get h(): number {\n    ${f}(() => {\n      void (super.x as any);\n    });\n    return 1;\n  },\n};\n(${object} as any).__proto__ = ${base};\nsetTimeout(() => {\n  void ${object}.h;\n});` };
  },
  methodsuper: (f, n) => {
    const cb = n("cb");
    const base = n("base");
    const object = n("o");
    return { head: `const ${cb} = () => `, close: `;\nconst ${base} = {\n  get x(): number {\n    return (this as any).g;\n  },\n};\nconst ${object} = {\n  get g(): number {\n    ${cb}();\n    return 1;\n  },\n  runIt() {\n    ${f}(() => {\n      void (super.x as any);\n    });\n  },\n};\n(${object} as any).__proto__ = ${base};\nsetTimeout(() => {\n  ${object}.runIt();\n});` };
  },
  protogetter: (f, n) => {
    const cb = n("cb");
    const base = n("base");
    const object = n("o");
    return { head: `const ${cb} = () => `, close: `;\nconst ${base} = {\n  get g(): number {\n    ${cb}();\n    return 1;\n  },\n};\nconst ${object} = {\n  __proto__: ${base},\n  get h(): number {\n    ${f}(() => {\n      void (super.g as any);\n    });\n    return 1;\n  },\n};\nsetTimeout(() => {\n  void ${object}.h;\n});` };
  },
  protogetterthis: (f, n) => {
    const cb = n("cb");
    const base = n("base");
    const object = n("o");
    return { head: `const ${cb} = () => `, close: `;\nconst ${base} = {\n  get g(): number {\n    ${cb}();\n    return 1;\n  },\n};\nconst ${object} = {\n  __proto__: ${base},\n  get h(): number {\n    ${f}(() => {\n      void (this as any).g;\n    });\n    return 1;\n  },\n};\nsetTimeout(() => {\n  void ${object}.h;\n});` };
  },
  protoassign: (f, n) => {
    const cb = n("cb");
    const base = n("base");
    const object = n("o");
    return { head: `const ${cb} = () => `, close: `;\nconst ${base} = {\n  get g(): number {\n    ${cb}();\n    return 1;\n  },\n};\nconst ${object} = {\n  get h(): number {\n    ${f}(() => {\n      void (super.g as any);\n    });\n    return 1;\n  },\n};\n(${object} as any).__proto__ = ${base};\nsetTimeout(() => {\n  void ${object}.h;\n});` };
  },
  protomethod: (f, n) => {
    const cb = n("cb");
    const base = n("base");
    const object = n("o");
    return { head: `const ${cb} = () => `, close: `;\nconst ${base} = {\n  m(): void {\n    ${cb}();\n  },\n};\nconst ${object} = {\n  __proto__: ${base},\n  get h(): number {\n    ${f}(() => {\n      (this as any).m();\n    });\n    return 1;\n  },\n};\nsetTimeout(() => {\n  void ${object}.h;\n});` };
  },
  ownmethod: (f, n) => {
    const cb = n("cb");
    const object = n("o");
    return { head: `const ${cb} = () => `, close: `;\nconst ${object} = {\n  m(): void {\n    ${cb}();\n  },\n  run(): void {\n    ${f}(() => {\n      this.m();\n    });\n  },\n};\nsetTimeout(() => {\n  ${object}.run();\n});` };
  },
  ownvalue: (f, n) => {
    const cb = n("cb");
    const object = n("o");
    return { head: `const ${cb} = () => `, close: `;\nconst ${object} = {\n  m(): void {\n    ${cb}();\n  },\n  run(): void {\n    ${f}(this.m);\n  },\n};\nsetTimeout(() => {\n  ${object}.run();\n});` };
  },
  foreignthis: (f, n) => {
    const cb = n("cb");
    const object = n("o");
    const method = n("g");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${method}(this: any) {\n  ${f}(() => {\n    this.m();\n  });\n}\nconst ${object} = {\n  m(): void {\n    ${cb}();\n  },\n  g: ${method},\n};\nsetTimeout(() => {\n  ${object}.g();\n});` };
  },
  foreignparam: (f, n) => {
    const cb = n("cb");
    const make = n("mk");
    const object = n("o");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${make}(f: any) {\n  return {\n    m(): void {\n      ${cb}();\n    },\n    g: f,\n  };\n}\nconst ${object} = ${make}(function (this: any) {\n  ${f}(() => {\n    this.m();\n  });\n});\nsetTimeout(() => {\n  ${object}.g();\n});` };
  },
  foreigncall: (f, n) => {
    const cb = n("cb");
    const make = n("mk");
    const object = n("o");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${make}() {\n  return function (this: any) {\n    ${f}(() => {\n      this.m();\n    });\n  };\n}\nconst ${object} = {\n  m(): void {\n    ${cb}();\n  },\n  g: ${make}(),\n};\nsetTimeout(() => {\n  ${object}.g();\n});` };
  },
  foreignalias: (f, n) => {
    const cb = n("cb");
    const source = n("src");
    const alias = n("h");
    const object = n("o");
    return { head: `const ${cb} = () => `, close: `;\nconst ${source} = {\n  a(this: any) {\n    ${f}(() => {\n      this.m();\n    });\n  },\n};\nconst { a: ${alias} } = ${source};\nconst ${object} = {\n  m(): void {\n    ${cb}();\n  },\n  ${alias},\n};\nsetTimeout(() => {\n  ${object}.${alias}();\n});` };
  },
  foreignarray: (f, n) => {
    const cb = n("cb");
    const fns = n("fns");
    const object = n("o");
    return { head: `const ${cb} = () => `, close: `;\nconst ${fns} = [\n  function (this: any) {\n    ${f}(() => {\n      this.m();\n    });\n  },\n];\nconst ${object} = {\n  m(): void {\n    ${cb}();\n  },\n  g: ${fns}[0],\n};\nsetTimeout(() => {\n  ${object}.g();\n});` };
  },
  lateassign: (f, n) => {
    const cb = n("cb");
    const object = n("o");
    const key = n("m");
    return { head: `const ${cb} = () => `, close: `;\nconst ${object} = {\n  get h(): number {\n    ${f}(() => {\n      (this as any).${key}();\n    });\n    return 1;\n  },\n  ${key}: undefined as any,\n};\n${object}.${key} = function () {\n  ${cb}();\n};\nsetTimeout(() => {\n  void ${object}.h;\n});` };
  },
  latesetter: (f, n) => {
    const cb = n("cb");
    const object = n("o");
    const key = n("m");
    return { head: `const ${cb} = () => `, close: `;\nconst ${object} = {\n  set h(value: number) {\n    ${f}(() => {\n      (this as any).${key}();\n    });\n  },\n  ${key}: undefined as any,\n};\n${object}.${key} = function () {\n  ${cb}();\n};\nsetTimeout(() => {\n  ${object}.h = 1;\n});` };
  },
  latenested: (f, n) => {
    const cb = n("cb");
    const object = n("o");
    const key = n("m");
    return { head: `const ${cb} = () => `, close: `;\nconst ${object} = {\n  get h(): number {\n    ${f}(() => {\n      (this as any).${key}.m();\n    });\n    return 1;\n  },\n  ${key}: undefined as any,\n};\n${object}.${key} = {\n  m(): void {\n    ${cb}();\n  },\n};\nsetTimeout(() => {\n  void ${object}.h;\n});` };
  },
  lateput: (f, n) => {
    const cb = n("cb");
    const object = n("o");
    const key = n("m");
    const put = n("put");
    return { head: `const ${cb} = () => `, close: `;\nconst ${object} = {\n  get h(): number {\n    ${f}(() => {\n      (this as any).${key}();\n    });\n    return 1;\n  },\n  ${key}: undefined as any,\n};\nfunction ${put}(target: any) {\n  target.${key} = function () {\n    ${cb}();\n  };\n}\n${put}(${object});\nsetTimeout(() => {\n  void ${object}.h;\n});` };
  },
  protoreceiver: (f, n) => {
    const cb = n("cb");
    const object = n("o");
    const key = n("m");
    const base = n("base");
    return { head: `const ${cb} = () => `, close: `;\nconst ${base} = {\n  get h(): number {\n    ${f}(() => {\n      (this as any).${key}();\n    });\n    return 1;\n  },\n};\nconst ${object} = {\n  __proto__: ${base},\n  ${key}(): void {\n    ${cb}();\n  },\n};\nsetTimeout(() => {\n  void (${object} as any).h;\n});` };
  },
  protocreate: (f, n) => {
    const cb = n("cb");
    const object = n("o");
    const key = n("m");
    const base = n("base");
    return { head: `const ${cb} = () => `, close: `;\nconst ${base} = {\n  get h(): number {\n    ${f}(() => {\n      (this as any).${key}();\n    });\n    return 1;\n  },\n};\nconst ${object}: any = Object.create(${base});\n${object}.${key} = function () {\n  ${cb}();\n};\nsetTimeout(() => {\n  void ${object}.h;\n});` };
  },
  protoclass: (f, n) => {
    const cb = n("cb");
    const object = n("o");
    const key = n("m");
    const klass = n("K");
    return { head: `const ${cb} = () => `, close: `;\nclass ${klass} {\n  ${key}: any;\n  get h(): number {\n    ${f}(() => {\n      this.${key}();\n    });\n    return 1;\n  }\n}\nconst ${object} = new ${klass}();\n${object}.${key} = function () {\n  ${cb}();\n};\nsetTimeout(() => {\n  void ${object}.h;\n});` };
  },
  protokey: (f, n) => {
    const cb = n("cb");
    const object = n("o");
    const base = n("base");
    const name = n("k");
    return { head: `const ${cb} = () => `, close: `;\nconst ${base} = {\n  get g(): number {\n    ${cb}();\n    return 1;\n  },\n};\nconst ${object}: any = {\n  get h(): number {\n    ${f}(() => {\n      void this.g;\n    });\n    return 1;\n  },\n};\nconst ${name} = \"__pro\" + \"to__\";\n${object}[${name}] = ${base};\nsetTimeout(() => {\n  void ${object}.h;\n});` };
  },
  lateself: (f, n) => {
    const cb = n("cb");
    const object = n("o");
    const key = n("m");
    const self = n("self");
    return { head: `const ${cb} = () => `, close: `;\nconst ${object} = {\n  get h(): number {\n    const ${self} = this;\n    ${f}(() => {\n      ${self}.${key}();\n    });\n    return 1;\n  },\n  ${key}: undefined as any,\n};\n${object}.${key} = function () {\n  ${cb}();\n};\nsetTimeout(() => {\n  void ${object}.h;\n});` };
  },
  latedestr: (f, n) => {
    const cb = n("cb");
    const object = n("o");
    const key = n("m");
    return { head: `const ${cb} = () => `, close: `;\nconst ${object} = {\n  get h(): number {\n    ${f}(() => {\n      const { ${key} } = this as any;\n      ${key}();\n    });\n    return 1;\n  },\n  ${key}: undefined as any,\n};\n${object}.${key} = function () {\n  ${cb}();\n};\nsetTimeout(() => {\n  void ${object}.h;\n});` };
  },
  latecomputed: (f, n) => {
    const cb = n("cb");
    const object = n("o");
    const key = n("m");
    const name = n("k");
    return { head: `const ${cb} = () => `, close: `;\nconst ${name} = \"${key}\";\nconst ${object} = {\n  get h(): number {\n    ${f}(() => {\n      (this as any)[${name}]();\n    });\n    return 1;\n  },\n  ${key}: undefined as any,\n};\n${object}.${key} = function () {\n  ${cb}();\n};\nsetTimeout(() => {\n  void ${object}.h;\n});` };
  },
  putparam: (f, n) => {
    const cb = n("cb");
    const put = n("put");
    const object = n("o");
    const key = n("m");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${put}(target: { ${key}(): void }) {\n  target.${key} = function () {\n    ${cb}();\n  };\n}\nconst ${object} = {\n  ${key}(): void {},\n};\n${put}(${object});\n${f}(() => {\n  ${object}.${key}();\n});` };
  },
  putparamany: (f, n) => {
    const cb = n("cb");
    const put = n("put");
    const object = n("o");
    const key = n("m");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${put}(target: any) {\n  target.${key} = function () {\n    ${cb}();\n  };\n}\nconst ${object}: any = {};\n${put}(${object});\n${f}(() => {\n  ${object}.${key}();\n});` };
  },
  putchain: (f, n) => {
    const cb = n("cb");
    const put = n("put");
    const outer = n("outer");
    const object = n("o");
    const key = n("m");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${put}(target: { ${key}(): void }) {\n  target.${key} = function () {\n    ${cb}();\n  };\n}\nfunction ${outer}(holder: { ${key}(): void }) {\n  ${put}(holder);\n}\nconst ${object} = {\n  ${key}(): void {},\n};\n${outer}(${object});\n${f}(() => {\n  ${object}.${key}();\n});` };
  },
};

/** The routes written at module scope: the review of #217's, whose question is the position of the function itself. */
const TOP_ROUTES = new Set(["mapinit", "mapdestr", "mapparam", "mapflat", "maptemplate", "maplogical", "mapawait", "mapassign", "maparg", "yieldof", "nbmap", "nbobj", "argsindex", "argsapply", "thenreturn", "thenconst", "thenlet", "thenfn", "thenmember", "refprop", "dynref", "chainconst", "chainassign", "chainrecv", "spreadref", "spreadvar", "childfn", "framechild", "framebare", "framemap", "framecall", "framerender", "frameiife", "framemapref", "framevoid", "frametypeof", "propsread", "propsrender", "getterparam", "selfgetter", "treerec", "propsjsx", "propsclass", "getterthis", "getterouter", "getterassign", "propsmethod", "getterpick", "gettersuper", "methodsuper", "protogetter", "protogetterthis", "protoassign", "protomethod", "ownmethod", "ownvalue", "foreignthis", "foreignparam", "foreigncall", "foreignalias", "foreignarray", "lateassign", "latesetter", "latenested", "lateput", "protoreceiver", "protocreate", "protoclass", "protokey", "lateself", "latedestr", "latecomputed", "putparam", "putparamany", "putchain"]);

/** The other names a route imports from the package, and the import lines of the program's own libraries it needs. */
const ROUTE_NAMES = { assign: ["config"] };
const ROUTE_LINES = { untrack: ['import { untrack } from "solid-js";'], dynamic: ['import { Dynamic } from "@solidjs/web";'], dynref: ['import { Dynamic } from "@solidjs/web";'] };

/** The files of `fz-step`: a manifest naming no Solid package, so the checker does not read its source. */
export function phPackageFiles() {
  const text = (lines) => `${lines.join("\n")}\n`;
  const hosts = {
    step: (run) => `void action(function* () {\n    ${run}\n  })();`,
    apply: (run) => `createRoot(() => {\n    createEffect(() => 1, () => {\n      ${run}\n    });\n  });`,
    leaf: (run) => `createRoot(() => {\n    createTrackedEffect(() => {\n      ${run}\n    });\n  });`,
  };
  const js = ['import { action, createEffect, createRoot, createTrackedEffect } from "solid-js";', ""];
  const dts = [];
  for (const host of PH_HOSTS) {
    const up = UPPER[host];
    js.push(`export const run${up} = (cb) => {\n  ${hosts[host]("cb();")}\n};`);
    js.push(`export const run${up}With = (o) => {\n  ${hosts[host]("o.run();")}\n};`);
    js.push(`export const run${up}All = (fns) => {\n  ${hosts[host]("for (const f of fns) f();")}\n};`);
    // A component body runs under an owner, where an action's call is its own diagnostic (ACTION_CALLED_IN_OWNED_SCOPE, the package's): the view runs its host from a microtask.
    js.push(`export const run${up}Get = (get) => {\n  ${hosts[host]("get()();")}\n};`);
    js.push(`export const run${up}Config = () => {\n  ${hosts[host]("config.onStep?.();")}\n};`);
    js.push(`export const ${up}View = (props) => {\n  queueMicrotask(() => {\n    ${hosts[host]("props.render?.();\n    if (typeof props.children === \"function\") props.children();")}\n  });\n  return null;\n};`);
    dts.push(`export declare const run${up}: (cb: () => void) => void;`);
    dts.push(`export declare const run${up}With: (o: { run: () => void }) => void;`);
    dts.push(`export declare const run${up}All: (fns: Array<() => void>) => void;`);
    dts.push(`export declare const run${up}Get: (get: () => () => void) => void;`);
    dts.push(`export declare const run${up}Config: () => void;`);
    dts.push(`export declare const ${up}View: (props: { render?: () => void; children?: () => void }) => any;`);
  }
  js.push("export const config = {};", "export const skipRun = (cb) => { void cb; };", "export const skipRunWith = (o) => { void o; };", "export const skipRunAll = (fns) => { void fns; };", "export const skipRunGet = (get) => { void get; };", "export const skipRunConfig = () => { void config; };", "export const SkipView = (props) => { void props; return null; };");
  dts.push("export declare const config: { onStep?: () => void };", "export declare const skipRun: (cb: () => void) => void;", "export declare const skipRunWith: (o: { run: () => void }) => void;", "export declare const skipRunAll: (fns: Array<() => void>) => void;", "export declare const skipRunGet: (get: () => () => void) => void;", "export declare const skipRunConfig: () => void;", "export declare const SkipView: (props: { render?: () => void; children?: () => void }) => any;");
  return {
    // No `dependencies` or `peerDependencies` on a Solid package: the build does not read this package's source (the program's own `solid-js` serves its import at run time).
    "package.json": `${JSON.stringify({ name: PH_PACKAGE, version: "1.0.0", type: "module", exports: { ".": { types: "./index.d.ts", default: "./index.js" } } }, null, 2)}\n`,
    "index.js": text(js),
    "index.d.ts": text(dts),
  };
}

/** The placement of `stmts` for a `ph-*` context (`{ body, view, packages, importLines }`), or undefined for any other. */
export function phContext(context, stmts, kit) {
  const parts = phParts(context);
  if (!parts) return undefined;
  const name = callee(parts.route, parts.host, parts.never);
  const frame = ROUTE_FRAMES[parts.route](name, kit.name);
  const framed = contain("frame", 9, stmts, frame);
  const atTop = TOP_ROUTES.has(parts.route);
  return {
    body: atTop ? [] : [framed],
    ...(atTop ? { top: [framed] } : {}),
    view: [],
    packages: { [PH_PACKAGE]: phPackageFiles() },
    importLines: [`import { ${[name, ...(ROUTE_NAMES[parts.route] ?? [])].join(", ")} } from "${PH_PACKAGE}";`, ...(ROUTE_LINES[parts.route] ?? [])],
  };
}

/** `{ [code]: { misuse, legal } }` for SPEC. */
export function phPlacement() {
  return Object.fromEntries(Object.entries(PH_CODES).map(([code, hosts]) => {
    const contexts = PH_CONTEXT_NAMES.filter((name) => hosts.includes(phParts(name).host));
    return [code, { misuse: contexts.filter((name) => PH_CONTEXTS[name] === "misuse"), legal: contexts.filter((name) => PH_CONTEXTS[name] === "legal") }];
  }));
}
