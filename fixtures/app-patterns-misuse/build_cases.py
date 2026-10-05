# Generates cases.json: app-code misuse patterns, each a misuse twin and a
# correct twin of one plain-Solid component. Run: python3 build_cases.py
import json
H = 'import { createEffect, createMemo, createSignal, createStore, createTrackedEffect, For, onCleanup, onSettled, Show } from "solid-js";\n'
def comp(body, extra=""):
    return H + extra + "\nexport default function App() {\n" + body + "\n}\n"
cases = []
def add(id, rule, export, why, misuse, correct, extra_m="", extra_c=None):
    cases.append({"id": id, "package": "solid-js", "version": "2.0.0-rc.13", "entrypoint": ".", "export": export,
                  "class": "app-pattern", "rule": rule, "why": why,
                  "misuse": comp(misuse, extra_m), "correct": comp(correct, extra_m if extra_c is None else extra_c)})

# ---- strict-read-untracked: reads in the component body
add("body-signal-read", "strict-read-untracked", "createSignal", "A signal read in the component body sees its value once.",
    "  const [count] = createSignal(1);\n  const doubled = count() * 2;\n  return <p>{doubled}</p>;",
    "  const [count] = createSignal(1);\n  return <p>{count() * 2}</p>;")
add("body-memo-read", "strict-read-untracked", "createMemo", "A memo read in the component body sees its value once.",
    "  const [count] = createSignal(1);\n  const doubled = createMemo(() => count() * 2);\n  const value = doubled();\n  return <p>{value}</p>;",
    "  const [count] = createSignal(1);\n  const doubled = createMemo(() => count() * 2);\n  return <p>{doubled()}</p>;")
add("body-store-read", "strict-read-untracked", "createStore", "A store property read in the component body sees its value once.",
    "  const [state] = createStore({ name: \"a\" });\n  const name = state.name;\n  return <p>{name}</p>;",
    "  const [state] = createStore({ name: \"a\" });\n  return <p>{state.name}</p>;")
add("body-signal-seeds-signal", "strict-read-untracked", "createSignal", "Seeding a signal from another in the body reads it untracked (the donegeon shape).",
    "  const [source] = createSignal(\"a\");\n  const [copy] = createSignal(source());\n  return <p>{copy()}</p>;",
    "  const [source] = createSignal(\"a\");\n  const copy = createMemo(() => source());\n  return <p>{copy()}</p>;")
add("body-read-in-condition", "strict-read-untracked", "createSignal", "An `if` on a signal in the body is evaluated once.",
    "  const [open] = createSignal(true);\n  if (open()) return <p>open</p>;\n  return <p>closed</p>;",
    "  const [open] = createSignal(true);\n  return <Show when={open()} fallback={<p>closed</p>}><p>open</p></Show>;")
add("body-read-through-local-helper", "strict-read-untracked", "createSignal", "A helper called from the body reads the signal in the body's extent.",
    "  const [count] = createSignal(1);\n  const label = () => `n=${count()}`;\n  const text = label();\n  return <p>{text}</p>;",
    "  const [count] = createSignal(1);\n  const label = () => `n=${count()}`;\n  return <p>{label()}</p>;")
add("body-read-through-hook", "strict-read-untracked", "createSignal", "A local hook that reads its argument in the call reads it in the body.",
    "  const [count] = createSignal(1);\n  const formatted = useFormatted(count);\n  return <p>{formatted}</p>;",
    "  const [count] = createSignal(1);\n  const formatted = useFormattedLive(count);\n  return <p>{formatted()}</p>;",
    extra_m="\nfunction useFormatted(read: () => number) {\n  return read().toFixed(1);\n}\n",
    extra_c="\nfunction useFormattedLive(read: () => number) {\n  return () => read().toFixed(1);\n}\n")
add("body-store-array-map", "strict-read-untracked", "createStore", "Mapping a store array in the body reads every element once.",
    "  const [state] = createStore({ items: [\"a\", \"b\"] });\n  const rows = state.items.map((item) => <li>{item}</li>);\n  return <ul>{rows}</ul>;",
    "  const [state] = createStore({ items: [\"a\", \"b\"] });\n  return <ul><For each={state.items}>{(item) => <li>{item()}</li>}</For></ul>;")
add("body-store-spread", "strict-read-untracked", "createStore", "Spreading a store into a plain object reads every property once.",
    "  const [state] = createStore({ a: 1, b: 2 });\n  const copy = { ...state };\n  return <p>{copy.a + copy.b}</p>;",
    "  const [state] = createStore({ a: 1, b: 2 });\n  return <p>{state.a + state.b}</p>;")
add("child-props-read-in-body", "strict-read-untracked", "createSignal", "A child reading its prop in its body sees the parent's signal once.",
    "  const [count] = createSignal(1);\n  return <Child value={count()} />;",
    "  const [count] = createSignal(1);\n  return <ChildLive value={count()} />;",
    extra_m="\nfunction Child(props: { value: number }) {\n  const value = props.value;\n  return <p>{value}</p>;\n}\n",
    extra_c="\nfunction ChildLive(props: { value: number }) {\n  return <p>{props.value}</p>;\n}\n")
add("child-props-destructured", "strict-read-untracked", "createSignal", "Destructuring props in the parameter reads them once.",
    "  const [count] = createSignal(1);\n  return <Child value={count()} />;",
    "  const [count] = createSignal(1);\n  return <ChildLive value={count()} />;",
    extra_m="\nfunction Child({ value }: { value: number }) {\n  return <p>{value}</p>;\n}\n",
    extra_c="\nfunction ChildLive(props: { value: number }) {\n  return <p>{props.value}</p>;\n}\n")
add("effect-apply-read", "strict-read-untracked", "createEffect", "A read in an effect's apply callback is untracked.",
    "  const [a] = createSignal(1);\n  const [b] = createSignal(2);\n  createEffect(() => a(), (value) => console.log(value + b()));\n  return <p>{a()}</p>;",
    "  const [a] = createSignal(1);\n  const [b] = createSignal(2);\n  createEffect(() => a() + b(), (value) => console.log(value));\n  return <p>{a()}</p>;")
add("effect-apply-store-read", "strict-read-untracked", "createEffect", "A store read in an effect's apply callback is untracked.",
    "  const [a] = createSignal(1);\n  const [state] = createStore({ name: \"x\" });\n  createEffect(() => a(), (value) => console.log(value, state.name));\n  return <p>{a()}</p>;",
    "  const [a] = createSignal(1);\n  const [state] = createStore({ name: \"x\" });\n  createEffect(() => [a(), state.name] as const, ([value, name]) => console.log(value, name));\n  return <p>{a()}</p>;")
add("for-children-body-read", "strict-read-untracked", "For", "A read in a For child's render function body is evaluated once per row.",
    "  const [items] = createSignal([\"a\", \"b\"]);\n  const [prefix] = createSignal(\">\");\n  return <ul><For each={items()}>{(item) => { const p = prefix(); return <li>{p}{item()}</li>; }}</For></ul>;",
    "  const [items] = createSignal([\"a\", \"b\"]);\n  const [prefix] = createSignal(\">\");\n  return <ul><For each={items()}>{(item) => <li>{prefix()}{item()}</li>}</For></ul>;")
add("show-children-body-read", "strict-read-untracked", "Show", "A read in a keyed Show child's function body is evaluated once.",
    "  const [user] = createSignal<{ name: string } | null>({ name: \"a\" });\n  const [suffix] = createSignal(\"!\");\n  return <Show when={user()}>{(u) => { const s = suffix(); return <p>{u().name}{s}</p>; }}</Show>;",
    "  const [user] = createSignal<{ name: string } | null>({ name: \"a\" });\n  const [suffix] = createSignal(\"!\");\n  return <Show when={user()}>{(u) => <p>{u().name}{suffix()}</p>}</Show>;")
add("callback-prop-called-in-render", "strict-read-untracked", "createSignal", "A render-prop callback the child calls in its body reads the signal during render.",
    "  const [count] = createSignal(1);\n  return <Render label={() => `n=${count()}`} />;",
    "  const [count] = createSignal(1);\n  return <RenderLive label={() => `n=${count()}`} />;",
    extra_m="\nfunction Render(props: { label: () => string }) {\n  const text = props.label();\n  return <p>{text}</p>;\n}\n",
    extra_c="\nfunction RenderLive(props: { label: () => string }) {\n  return <p>{props.label()}</p>;\n}\n")
add("on-settled-read", "strict-read-untracked", "onSettled", "A read in an onSettled callback is untracked.",
    "  const [count] = createSignal(1);\n  onSettled(() => { console.log(count()); });\n  return <p>{count()}</p>;",
    "  const [count] = createSignal(1);\n  createEffect(() => count(), (value) => { console.log(value); });\n  return <p>{count()}</p>;")

# ---- reactive-write-in-owned-scope
add("write-in-memo", "reactive-write-in-owned-scope", "createMemo", "A signal written inside a memo's compute.",
    "  const [count] = createSignal(1);\n  const [seen, setSeen] = createSignal(0);\n  const doubled = createMemo(() => { setSeen(1); return count() * 2; });\n  return <p>{doubled()} {seen()}</p>;",
    "  const [count] = createSignal(1);\n  const doubled = createMemo(() => count() * 2);\n  return <p>{doubled()}</p>;")
add("write-in-component-body", "reactive-write-in-owned-scope", "createSignal", "A signal written directly in the component body.",
    "  const [count, setCount] = createSignal(1);\n  setCount(2);\n  return <p>{count()}</p>;",
    "  const [count] = createSignal(2);\n  return <p>{count()}</p>;")
add("write-in-jsx-expression", "reactive-write-in-owned-scope", "createSignal", "A signal written inside a tracked JSX expression.",
    "  const [count, setCount] = createSignal(1);\n  return <p>{(setCount(2), count())}</p>;",
    "  const [count, setCount] = createSignal(1);\n  return <button onClick={() => setCount(2)}>{count()}</button>;")
add("write-in-effect-compute", "reactive-write-in-owned-scope", "createEffect", "A signal written inside an effect's compute function.",
    "  const [count] = createSignal(1);\n  const [copy, setCopy] = createSignal(0);\n  createEffect(() => { setCopy(count()); return count(); }, () => {});\n  return <p>{copy()}</p>;",
    "  const [count] = createSignal(1);\n  const [copy, setCopy] = createSignal(0);\n  createEffect(() => count(), (value) => { setCopy(value); });\n  return <p>{copy()}</p>;")

# ---- missing-owner
add("cleanup-after-await", "missing-owner", "onCleanup", "onCleanup after an await runs with no owner.",
    "  const [count] = createSignal(1);\n  void (async () => { await Promise.resolve(); onCleanup(() => console.log(\"bye\")); })();\n  return <p>{count()}</p>;",
    "  const [count] = createSignal(1);\n  onCleanup(() => console.log(\"bye\"));\n  void (async () => { await Promise.resolve(); })();\n  return <p>{count()}</p>;")
add("effect-in-timeout", "missing-owner", "createEffect", "An effect created from a timer has no owner.",
    "  const [count] = createSignal(1);\n  setTimeout(() => { createEffect(() => count(), (v) => console.log(v)); }, 0);\n  return <p>{count()}</p>;",
    "  const [count] = createSignal(1);\n  createEffect(() => count(), (v) => console.log(v));\n  return <p>{count()}</p>;")
add("cleanup-in-timeout", "missing-owner", "onCleanup", "onCleanup from a timer has no owner.",
    "  const [count] = createSignal(1);\n  setTimeout(() => { onCleanup(() => console.log(\"bye\")); }, 0);\n  return <p>{count()}</p>;",
    "  const [count] = createSignal(1);\n  onCleanup(() => console.log(\"bye\"));\n  return <p>{count()}</p>;")
add("effect-in-promise-then", "missing-owner", "createEffect", "An effect created in a promise continuation has no owner.",
    "  const [count] = createSignal(1);\n  void Promise.resolve().then(() => { createEffect(() => count(), (v) => console.log(v)); });\n  return <p>{count()}</p>;",
    "  const [count] = createSignal(1);\n  createEffect(() => count(), (v) => console.log(v));\n  void Promise.resolve();\n  return <p>{count()}</p>;")

# ---- leaf-owner-forbidden-call
add("cleanup-in-tracked-effect", "leaf-owner-forbidden-call", "createTrackedEffect", "onCleanup inside createTrackedEffect, a leaf owner.",
    "  const [count] = createSignal(1);\n  createTrackedEffect(() => { count(); onCleanup(() => console.log(\"bye\")); });\n  return <p>{count()}</p>;",
    "  const [count] = createSignal(1);\n  createTrackedEffect(() => { count(); return () => console.log(\"bye\"); });\n  return <p>{count()}</p>;")
add("signal-in-tracked-effect", "leaf-owner-forbidden-call", "createTrackedEffect", "A primitive created inside createTrackedEffect, a leaf owner.",
    "  const [count] = createSignal(1);\n  createTrackedEffect(() => { count(); createSignal(0); });\n  return <p>{count()}</p>;",
    "  const [count] = createSignal(1);\n  const [other] = createSignal(0);\n  createTrackedEffect(() => { count(); other(); });\n  return <p>{count()}</p>;")
add("memo-in-on-settled", "leaf-owner-forbidden-call", "onSettled", "A memo created inside onSettled, a leaf owner.",
    "  const [count] = createSignal(1);\n  onSettled(() => { createMemo(() => 1); });\n  return <p>{count()}</p>;",
    "  const [count] = createSignal(1);\n  const one = createMemo(() => 1);\n  onSettled(() => { console.log(\"settled\"); });\n  return <p>{count()}{one()}</p>;")


# ---- reads through a helper, in the shapes ADR 0201 proves
add("effect-apply-read-through-helper", "strict-read-untracked", "createEffect", "A helper called from an effect's apply callback reads a signal in that untracked extent.",
    "  const [a] = createSignal(1);\n  const [b] = createSignal(2);\n  const total = (value: number) => value + b();\n  createEffect(() => a(), (value) => console.log(total(value)));\n  return <p>{a()}</p>;",
    "  const [a] = createSignal(1);\n  const [b] = createSignal(2);\n  createEffect(() => a() + b(), (value) => console.log(value));\n  return <p>{a()}</p>;")
add("hook-reads-own-signal", "strict-read-untracked", "createSignal", "A hook called in the body reads a signal it created, in its own body.",
    "  const state = useCounter();\n  return <p>{state.count()}</p>;",
    "  const state = useCounterLive();\n  return <p>{state.count()}</p>;",
    extra_m="\nfunction useCounter() {\n  const [count] = createSignal(1);\n  console.log(count());\n  return { count };\n}\n",
    extra_c="\nfunction useCounterLive() {\n  const [count] = createSignal(1);\n  createEffect(() => count(), (value) => console.log(value));\n  return { count };\n}\n")
add("hook-reads-accessor-argument", "strict-read-untracked", "createSignal", "A hook called in the body reads the accessor it receives, in its own body.",
    "  const [target] = createSignal(1);\n  useSensor(target);\n  return <p>{target()}</p>;",
    "  const [target] = createSignal(1);\n  useSensorLive(target);\n  return <p>{target()}</p>;",
    extra_m="\nfunction useSensor(read: () => number) {\n  const start = read();\n  console.log(start);\n}\n",
    extra_c="\nfunction useSensorLive(read: () => number) {\n  createEffect(() => read(), (value) => console.log(value));\n}\n")

json.dump({"format": "solid-checker-app-patterns-misuse", "version": 1,
           "note": "Plain-Solid misuse patterns from application code, each with a correct twin. Run through benchmarks/reviewed-package-models/development/misuse-runtime-ledger.mjs --cases; built by build_cases.py.",
           "cases": cases}, open("cases.json", "w"), indent=1)
print(len(cases), "cases")
