# action-called-in-owned-scope

`SC2002` · **error** · violation

An `action` is invoked inside an owned scope — a component body, a
`createRoot` body, or a children-capable computation.

## What it does

Flags calls to functions created with `action()` when they execute under a live
children-capable owner: a component body, a `createRoot` body, a memo, or an
effect's compute function. Actions may be invoked from event handlers, effect apply callbacks,
and the children-forbidden leaf scopes `onSettled` and `createTrackedEffect` —
the runtime's action guard uses the same owner test as the write guard and
explicitly exempts leaf imperative scopes. An invocation no fact places (a
helper with no resolved caller, a JSX attribute function, code after an
`await`) is **uncertifiable**. A helper is resolved by its exact binding: a
declaration, or an arrow or function expression bound to a `const` identifier
(`const useMenu = () => ...`), is proven to run where its direct callers run.
A `let` nothing reassigns is exact too (ADR 0516); a written `let` and a
`var` are not, so a helper held in one stays uncertifiable.
An action call in a lazy memo (`createMemo(fn, { lazy: true })`) no proven
read of its accessor computes, or in code no execution reaches (ADRs 0458,
0578; a callback a standard-library iteration of an empty literal never runs,
ADR 1019), is uncertifiable (ADR 0576).
An action call in a function no call edge reaches but that may run (called
from a getter, `toString`, an iterator, a tag, a `Map` slot, a method, a
constructor or an instance field) is uncertifiable too, never certified (ADR
0715): Chrome throws when a render runs it. A construct-only class (one `new`,
after it in the same non-async body, outside JSX; no `this`, `super`,
`extends`) runs its constructor, its instance fields and the method called on
that `new` there: a call written directly in one takes that body's role (ADR
1022). Module initialization (outside
every function, a static block or field) has no owner and is silent.
An action handed to a project function calling that parameter in its body is
called at that call; U if only on some paths (ADR 0929).

A `createRoot` body runs during the call with the new root as the ambient
owner, and the action guard has no root exemption on any release: an action
called directly in the body, or in a memo compute nested in it, throws
`ACTION_CALLED_IN_OWNED_SCOPE` (probed on every published `2.0.0-rc.0` to
`rc.9` triple, dev client builds). A function passed to `createRoot` by name
is the root body too, resolved by symbol to its same-file declaration. A
closure bound to a name in a root body runs under the root where it is called,
even when a cleanup or a handler holds the root call (ADR 0471). An
effect apply, `onSettled` or an event listener inside the root keeps its own,
legal, answer.

`untrack` and `flush(fn)` are **not** escape hatches: each runs its callback
inline and keeps the owner context, so an action invoked inside `untrack(...)`
or `flush(() => ...)` within a memo, component body or root body still throws
`ACTION_CALLED_IN_OWNED_SCOPE` at runtime.

An action call that no row, discard or named silence ends is uncertifiable: `action-unaccounted` (ADR 0936).

An action called where it is created, with no binding (`action(function* () {})()`), is that action's call
and is placed as a bound one is (ADR 0972).

A call into code of an installed package the build does not read as source (a workspace package linked into
`node_modules` is one, and so is JavaScript beside a project declaration file that the program does not analyze) may call
`action` inside the package, where the program shows none (ADR 0972). The code a call runs is where its callee's value came from (the
import its root names, through the calls and project bindings that made it), never where its signature type is declared. Each such call,
`new`, tagged template, render of the package's component and function handed on as a value (an import binding or a member of an import
object passed as an argument, a `{ shorthand }` property, a namespace member, a project binding that holds one) is a site, and so is a call of a global the standard library declares that an installed package writes at run time (`globalThis.setTimeout = …`: the type is sound for the declaration, the write comes from the scan; a writer under a key nothing in its file decides, `window[key] = v` with a key from a call or a parameter, may have written any global, so every such call is a site), a call of a method the standard library declares where an installed package writes a member of that name on a built-in's prototype (`Float32Array.prototype.reverse = …`, through a binding nothing reassigns, `Object.defineProperty`, `Object.assign` or `Reflect.set`; a prototype handed on, generated code, the global object handed on and unread code may write any: ADR 1108; part 2 adds the writer shapes `Object.getPrototypeOf(X).prototype`, a constant key, `const { prototype } = X`, a parameter's `C.prototype`, `event.view.X.prototype`, a binding that holds a built-in and a value-reached prototype given to a writer; a call that runs the written name another way is a site too, ADR 1114 family 3: `.call`, `.apply`, `.bind`, `Reflect.apply`, a destructured binding, a receiver typed by a project interface or `any`, a bare call of a method the window inherits, the library's implicit calls (`test` runs `exec`, `JSON.stringify` `toJSON`, the collection constructors `set` and `add`, `toString` `join`), a template's substitution, a member read of a written name (an accessor) and, under an any-name writer, a library method, a binding of one and a collection constructor given a value, and every iteration (a spread, `for…of`, an array pattern, `yield*`), and a member call no declaration resolves; slice E: under any prototype writer every member call, every call, constructor and tag the library declares, every converting operator, array pattern, `await`, `using` and JSX value is a site, whatever the table of implicit calls names; slice F: there no package is clear, and each JSX template root is a site under a writer of `cloneNode` or `importNode`, which the runtime clones it with; slice G: under an any-name writer every member read, object pattern and `in`, every assignment to a member and computed key, any operation whose native path may read a name written by name, and an async function's returned value; and a function handed to a standard call whose member a package writes has no placed role), and a value whose origin the walk stopped following at a limit (`origin-limit`: an array of more than 256 elements, a chain of more than 48 hops), and the component a
`lazy(() => import("pkg"))` makes, a call of a global a declaration file declares (a package of global declarations only, a project `declare function`,
a member a project `interface Window` adds: the declaration does not say who provides the global, and a package may) and a member of an object the package made,
evaluated for its value or read by an object pattern (`const { now } = made`), where the package's closure may define an accessor under that name (`void clock.now`: a getter runs where it is read; ADR 0974): where its role may run under an owner or nothing
places it, it is **uncertifiable** (`package-may-call-action:<why>`), never a violation, unless the package's
source, walked from every entry through every import, `require` and literal `import()`, names no Solid
package (a `@solidjs/*` package counts) and loads nothing by a computed name, generates no code (`eval`,
`Function`, a string timer) and reads no global the pinned TypeScript's `lib.es*`/`lib.dom*` files do not
declare, however the global object is reached (`window.self.x`, `globalThis.globalThis.x` and `document.defaultView.x` are `window.x`). A package that hands the global object on (an argument, an assignment, a return, `this` at the top of a script, `window.valueOf()`, `defaultView`, an event's `view` or a message's `source` of any object, also read by an object pattern, ADR 1108; a string-literal `in` test, a type query and a `defaultView`, `view` or `source` the program's types prove holds no object off a receiver typed as no document, `UIEvent` or `MessageEvent` respectively are no use of it, ADRs 1077 and 1108) or builds code (`eval`, `Function`, `window.eval(…)`, a string timer, `.constructor("…")`) may write any global, an accessor under any name included: a call of a global the library declares, a read of `window` and a read of a global are sites wherever it is loaded (ADR 0974); a writer reached through an alias, `bind`, `call`, `apply` or `Reflect.apply` is judged like a direct call, by its target. So may a package whose closure holds code nobody read (a load by a computed name, a load no installed file answers, an unparseable file, a native module, an unread manifest, files past the closure limit; ADR 1002): what its read files write says nothing of the rest. A member read needs no site where the package's
whole closure is read and holds no accessor (class, object literal or `defineProperty` getter and setter, a `Proxy`) under a name of the chain. Missing source, a native module, an unparseable file and the WASM `checkSync` entry (no filesystem)
stay uncertifiable. The package scan discharges immediate static `Function.toString.call` / `.apply` and the data reads `Function.name` / `.length` (ADR 1062): these generate no code. A held `Function.toString`, computed member, or `Function.call` / `.apply` / `.bind` remains undecided. A package the build reads as source is analyzed, and a row in its code stands at the
project's entry (ADR 0949); the dialect's own runtime, the standard library (and a member of a standard-library global the library declares), a reviewed test runner's
registration, a call the reviewed reading of vitest 4.1's `expect` and `vi` clears (ADR 0975: pinned to the digests of the files read, for the matchers and values it names) and a function the project itself defines are no
sites.

## Why is this bad?

Invoking an action starts a write transaction: optimistic writes apply, async work
runs inside the transition, and `refresh` re-derives state when it settles. Started
under a children-capable owner, that transaction invalidates the very graph that is
being tracked — re-triggering the scope that called it, exactly the feedback loop
Solid 2.0 forbids for plain setters. Leaf scopes have no children to re-trigger,
which is why the runtime leaves them legal.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const save = action(function* (todo) {
  setOptimisticTodos((s) => {
    s.push(todo);
  });
  yield api.add(todo);
  refresh(todos);
});

function TodoList() {
  save(defaultTodo); // Called during component setup — starts a transaction under a live owner.
  return <For each={todos()}>{(todo) => <Row todo={todo} />}</For>;
}

// A root body runs under the root, a children-capable owner.
createRoot(() => save(defaultTodo));
```

Examples of **correct** code for this rule:

```tsx
function TodoList() {
  return (
    <>
      <button onClick={() => save(defaultTodo)}>Add</button>
      <For each={todos()}>{(todo) => <Row todo={todo} />}</For>
    </>
  );
}

// Leaf scopes are legal action sites — the guard exempts them:
createTrackedEffect(() => {
  if (shouldAutosave()) save(draft());
});
```

## How to fix

Call the action from an event handler, `onSettled`, `createTrackedEffect`, or
another imperative boundary; wrapping the call in `untrack` does not help.
If the goal is loading data reactively rather than mutating it, an action is the
wrong tool: return the Promise from a computation (`createMemo(() => fetchX())`)
and read it under a `<Loading>` boundary.

## Related

- [no-write-in-pure-computation](no-write-in-pure-computation.md) — the same constraint for setters
- [async-outside-loading-boundary](async-outside-loading-boundary.md) — reactive data loading

Generators are placed as for [no-write-in-pure-computation](no-write-in-pure-computation.md) (ADRs 0851, 0853).

## The product realm (ADR 1145)

A package that writes a built-in's prototype is judged against the page that runs the site. Where the build configuration says which modules the page loads (an `index.html`'s module scripts or a package's `exports`, with the Solid plugin (an installed closure ADR 0270 pins) the only Vite plugin, no other bundler's configuration or Vite config file, no `package.json` script that runs `vite` (or forwards arguments to a script that does) other than plainly in the project directory, no second page that can run code, every load of the entries' closure resolved the way Vite resolves it, and the packages the loaded packages' manifests depend on held (a Solid package's dependencies, the Solid plugin's dev-serve diagnostics bridge's `@solidjs/diagnostics`): an extension sibling, package `imports`, a tsconfig option or `paths` Vite does not share, and a `?url` or `?raw` on a script end the read), a site in a file those entries load is judged against the writers the entries load (and Vite's own packages and the config's imports, in either build), not against a package only a test, a story, a script or a config loads. The site is not dropped: it ends in the named silence `package_action_calls.package_action_calls.product-realm`, so the proof log names it. Any doubt leaves every site judged against every writer, and a test's own sites keep every writer. The premises and what is not scoped are in ADR 1145 and `docs/open-work.md`.

## Vite's reviewed dev client (ADR 1144)

The package named `vite` is read from its Node entry, where the walk finds a writer of any prototype member; that code runs in the dev server, not in the page. Where no program file loads `vite` or a `vite/*` subpath at run time, no other installed package depends on it, the build is a development build in a browser, every Vite config reads in the closed grammar with no plugin but Solid's (an installed closure ADR 0270 pins, whose `define` keys are listed and each a data-only new global), and the installed manifest and dev client are a release whose bytes were read (Vite 8.3.0, 7.3.1), a site whose only prototype writer is that root ends in the named silence `package_action_calls.package_action_calls.reviewed-host`. The package keeps its global writes: a read of a global the client writes (`console`, whose `error` and `warn` Vite 8 replaces; a define) is still a site. Any other release, a patched file or any doubt keeps every row.

## Endings by code (ADR 0968)

An action call is accounted for, for this rule's Chrome code, only by an ending that answers that code: a row of its own table, an SC9012, the compiler's discard, a finding of a rule the dialect says claims the code, or a named silence that names it. A row of another table or a finding of another rule at the same place answers another code and ends nothing here, so this rule's own uncertifiable row (`action-unaccounted`) stands beside it: never a violation, and it yields to any row of the rule that stands. A site whose rule claims no code here, or whose code has no policy in the dialect, is not ended by what stands at it.
