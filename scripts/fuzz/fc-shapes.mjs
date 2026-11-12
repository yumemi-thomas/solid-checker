// The flush-class shapes (batch 11, the review of #213): the census class of the code around a flush is not a statement about the flush's own stack when the flush is written where its
// class does not run it. Two shapes, each a function the program enters again, or elsewhere, than the scheduler's first run:
//
//   fc-<form>-<host>          misuse   the flush runs on the host's stack, in the host's callback
//   fc-<form>-<host>-never    legal    the same text, the host never runs the flush (the name never called, the function never called)
//     forms   named           `setTimeout(function tick(again?: boolean) { <site> if (!again) { <host calls tick(true)> } })`: a named function expression is entered again by its name, from
//                             the stack of whatever calls the name. The class of the scheduler's callback (FreshStack, and a runner's TestRunner) said "an empty stack" of the first run only.
//             pdefault        `late = (x = <site>) => x;` written in a timer's callback, `late()` called by the host: a default value runs wherever its function is called. The class of the
//                             code around the nested function (the timer's callback) said nothing of that call.
//             pdefault-await  the same, the assignment written after an `await` of an async function (the class of the code after an await said "a microtask" of the code after it).
//             field           `late = class { x = <site>; }` written in a timer's callback, `new late()` in the host: a class field's initializer runs at construction, not where the class is written.
//             gencarrier      `late = [1].map(function* () { <site> })` written in a timer's callback, `late[0].next()` in the host: the carrier only creates the generator object, its body runs
//                             at `.next()` (the re-review of #213, N1 and N2).
//             jsxprop         `<Child v={<site>} />` created in a timer's callback, `props.v` read in the host: a prop written in JSX is a getter, it runs where the component reads it (N3).
//             recvthen        `recv.catch(() => { <site> })` written in the host's callback, `recv` an instance of a Promise subclass whose `then` runs the handler at once: `catch` runs its callback
//                             through the receiver's `then` (ECMAScript 27.2.5.1), so the callback is run on the host's stack (the second re-review of #213, N5). The twin's `then` runs nothing.
//   hosts   effect-apply (an effect's apply: FLUSH_IN_EFFECT_CALLBACK), action (an action's step: FLUSH_IN_ACTION), tracked (a leaf owner's callback: FLUSH_IN_TRACKED_QUEUE_CALLBACK)
//
// Chrome (rc.21, the real oracle) confirmed the false clean on #213's head e70b560f2 (a typed reason `FlushSafe::FreshStack` ended the site): `named` in an action's step and in an effect's apply,
// and `pdefault` in an effect's apply. What Chrome reports in each cell is what the judge compares the checker with (a `-misuse` cell Chrome keeps quiet on is no false clean). Recall contexts, the
// direct binding bare (`fcShape`), not crossed by the census, out of api-fire's plan. A test runner's callback has the same two conditions, and the generator has no runner (patterns/flushclass
// holds the shapes in plain programs; fixtures/reactive-ir/flush-universe-class holds the runner's).
import { contain } from "./model.mjs";

export const FC_FORMS = ["named", "pdefault", "pdefault-await", "field", "gencarrier", "jsxprop", "recvthen"];
export const FC_HOSTS = ["effect-apply", "action", "tracked"];
/** The hosts each code is judged in: the code the host's callback reports for a flush in it. */
export const FC_CODES = {
  FLUSH_IN_EFFECT_CALLBACK: ["effect-apply"],
  FLUSH_IN_ACTION: ["action"],
  FLUSH_IN_TRACKED_QUEUE_CALLBACK: ["tracked"],
};
/** `{ [context]: "misuse" | "legal" }`. */
export const FC_CONTEXTS = Object.fromEntries(FC_FORMS.flatMap((form) => FC_HOSTS.flatMap((host) => [[`fc-${form}-${host}`, "misuse"], [`fc-${form}-${host}-never`, "legal"]])));
export const FC_CONTEXT_NAMES = Object.keys(FC_CONTEXTS);

const NAME = new RegExp(`^fc-(${[...FC_FORMS].sort((a, b) => b.length - a.length).join("|")})-(${FC_HOSTS.join("|")})(-never)?$`);
/** `{ form, host, never }` of an `fc-*` context name, or null. */
export const fcParts = (context) => {
  const match = NAME.exec(context);
  return match ? { form: match[1], host: match[2], never: Boolean(match[3]) } : null;
};

/** The base shapes an `fc-*` cell takes: the direct binding, no layer. */
export function fcShape(context, shape) {
  return !fcParts(context) || (shape.binding === "direct" && (shape.layer ?? "none") === "none");
}

/**
 * The host's own text: the lines that create the host so that its callback runs `use` (a statement), and the names they import. `trigger` is what makes it run once the function exists
 * (the action's call, the signal's write an effect reads). `root` wraps the creation of an effect in a root, for text written where no owner is current (a timer's callback).
 */
function hostLines(host, n, use, root) {
  const sig = n("fs");
  const setSig = n("fsw");
  const run = n("fr");
  const wrap = (lines) => (root ? [`createRoot(() => {`, ...lines.map((line) => `  ${line}`), `});`] : lines);
  const rooted = root ? ["createRoot"] : [];
  // A root's effect runs on creation: no write is needed, and none is named.
  const signal = root ? `const [${sig}] = createSignal(0);` : `const [${sig}, ${setSig}] = createSignal(0);`;
  switch (host) {
    case "effect-apply": return { lines: [signal, ...wrap([`createEffect(`, `  () => ${sig}(),`, `  () => {`, `    ${use}`, `  },`, `);`])], trigger: root ? null : `${setSig}(1);`, imports: ["createEffect", ...rooted] };
    case "tracked": return { lines: [signal, ...wrap([`createTrackedEffect(() => {`, `  ${sig}();`, `  ${use}`, `});`])], trigger: root ? null : `${setSig}(1);`, imports: ["createTrackedEffect", ...rooted] };
    case "action": return { lines: [`const ${run} = action(function* () {`, `  ${use}`, `});`], trigger: `void ${run}();`, imports: ["action"] };
    default: throw new Error(`no flush-class host ${host}`);
  }
}

/**
 * The frame of a form (model.mjs FRAME_CONTAINER_KINDS): `head` precedes the body, `close` follows it, `after` are the lines inside the body after the statements, `mode: "expr"` writes the one
 * expression as a parameter's default, and `imports` the names the raw text needs.
 */
const FRAMES = {
  named: (n, host, never) => {
    const t = n("tick");
    const again = n("again");
    // The host calls the name from its own stack: the action's step, the effect's apply, the leaf callback. In the twin it runs a statement that calls nothing.
    const { lines, trigger, imports } = hostLines(host, n, never ? "void 0;" : `${t}(true);`, true);
    return {
      head: `setTimeout(function ${t}(${again}?: boolean) `,
      close: ");",
      after: [`if (!${again}) {`, ...[...lines, ...(trigger ? [trigger] : [])].map((line) => `  ${line}`), "}"],
      imports,
    };
  },
  pdefault: (n, host, never) => {
    const late = n("late");
    const { lines, trigger, imports } = hostLines(host, n, never ? `void ${late};` : `${late}();`, false);
    return {
      head: `let ${late}: (x?: void) => void = () => {};\n${lines.join("\n")}\nsetTimeout(() => {\n  ${late} = (x = `,
      close: `) => x;\n  ${trigger}\n});`,
      mode: "expr",
      imports,
    };
  },
  field: (n, host, never) => {
    const klass = n("Late");
    const { lines, trigger, imports } = hostLines(host, n, never ? `void ${klass};` : `if (${klass}) new ${klass}();`, false);
    return {
      head: `let ${klass}: undefined | (new () => object);\n${lines.join("\n")}\nsetTimeout(() => {\n  ${klass} = class {\n    x = `,
      close: `;\n  };\n  ${trigger}\n});`,
      mode: "expr",
      imports,
    };
  },
  gencarrier: (n, host, never) => {
    const late = n("late");
    const { lines, trigger, imports } = hostLines(host, n, never ? `void ${late};` : `${late}?.[0]?.next();`, false);
    return {
      head: `let ${late}: undefined | Generator<void, void, unknown>[];\n${lines.join("\n")}\nsetTimeout(() => {\n  ${late} = [1].map(function* () `,
      close: `);\n  ${trigger}\n});`,
      nest: 1,
      imports,
    };
  },
  jsxprop: (n, host, never) => {
    const child = n("Child");
    const read = n("read");
    const sig = n("fs");
    const run = n("fr");
    const use = never ? `void ${read};` : `${read}();`;
    // The component reads the prop in the host's callback; the host is created inside the component, which a root owns.
    const inside = {
      "effect-apply": { pre: [`const [${sig}] = createSignal(0);`], body: [`createEffect(`, `  () => ${sig}(),`, `  () => {`, `    ${use}`, `  },`, `);`], imports: ["createEffect"] },
      tracked: { pre: [`const [${sig}] = createSignal(0);`], body: [`createTrackedEffect(() => {`, `  ${sig}();`, `  ${use}`, `});`], imports: ["createTrackedEffect"] },
      action: { pre: [], body: [`const ${run} = action(function* () {`, `  ${use}`, `});`, `setTimeout(() => void ${run}());`], imports: ["action"] },
    }[host];
    return {
      head: `${inside.pre.join("\n")}${inside.pre.length ? "\n" : ""}function ${child}(props: { v: void }) {\n  const ${read} = () => props.v;\n${inside.body.map((line) => `  ${line}`).join("\n")}\n  return <p>c</p>;\n}\nsetTimeout(() => {\n  document.body.append((<${child} v={`,
      close: `} />) as unknown as Node);\n});`,
      mode: "expr",
      imports: inside.imports,
    };
  },
  recvthen: (n, host, never) => {
    const sync = n("Sync");
    const recv = n("recv");
    const sig = n("fs");
    const run = n("fr");
    // A promise whose `then` runs the rejection handler at once: `catch(cb)` is `this.then(undefined, cb)`. In the twin `then` runs nothing, so the handler never runs.
    const klass = [
      `class ${sync}<T> extends Promise<T> {`,
      `  override then<A = T, B = never>(ok?: ((value: T) => A | PromiseLike<A>) | null, bad?: ((reason: unknown) => B | PromiseLike<B>) | null): Promise<A | B> {`,
      `    void ok;`,
      `    ${never ? "void bad;" : "bad?.(undefined);"}`,
      `    return this as unknown as Promise<A | B>;`,
      `  }`,
      `}`,
      `const ${recv} = new ${sync}<void>(() => {});`,
    ].join("\n");
    const open = `${recv}.catch(() => `;
    // The host's callback calls `catch`; the site is written in its callback, which `then` runs on the host's stack.
    const hosts = {
      "effect-apply": { head: `const [${sig}] = createSignal(0);\ncreateEffect(\n  () => ${sig}(),\n  () => {\n    ${open}`, close: `);\n  },\n);`, imports: ["createEffect"] },
      tracked: { head: `const [${sig}] = createSignal(0);\ncreateTrackedEffect(() => {\n  ${sig}();\n  ${open}`, close: `);\n});`, imports: ["createTrackedEffect"] },
      action: { head: `const ${run} = action(function* () {\n  ${open}`, close: `);\n});\nsetTimeout(() => void ${run}());`, imports: ["action"] },
    }[host];
    return { head: `${klass}\n${hosts.head}`, close: hosts.close, imports: hosts.imports };
  },
  "pdefault-await": (n, host, never) => {
    const late = n("late");
    const make = n("make");
    const { lines, trigger, imports } = hostLines(host, n, never ? `void ${late};` : `${late}();`, false);
    return {
      head: `let ${late}: (x?: void) => void = () => {};\n${lines.join("\n")}\nconst ${make} = async () => {\n  await Promise.resolve();\n  ${late} = (x = `,
      close: `) => x;\n  ${trigger}\n};\nvoid ${make}();`,
      mode: "expr",
      imports,
    };
  },
};

/** The placement of `stmts` for an `fc-*` context (`{ body, view }`), or undefined for any other. */
export function fcContext(context, stmts, kit) {
  const parts = fcParts(context);
  if (!parts) return undefined;
  const frame = FRAMES[parts.form](kit.name, parts.host, parts.never);
  return { body: [contain("frame", 9, stmts, frame)], view: [] };
}

/** `{ [code]: { misuse, legal } }` for SPEC. */
export function fcPlacement() {
  return Object.fromEntries(Object.entries(FC_CODES).map(([code, hosts]) => {
    const contexts = FC_CONTEXT_NAMES.filter((name) => hosts.includes(fcParts(name).host));
    return [code, { misuse: contexts.filter((name) => FC_CONTEXTS[name] === "misuse"), legal: contexts.filter((name) => FC_CONTEXTS[name] === "legal") }];
  }));
}
