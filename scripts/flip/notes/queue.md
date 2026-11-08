# #173 queue: every report is in

Owner's rules. What is not proven is a clear uncertifiable with a named blocker, never a silent clean, however
rare. Prefer a proved violation where the fact is local, because uncertifiable warns nobody. Every real false
positive and false clean goes in #173. One family at a time, fuzzer first: pattern, old fails, fix, new passes,
`rediscovery.json` entry with ids, local commit with the evidence. No `git push`. No full grid run or final order
until this queue is done. Reproduce each in real Chrome yourself, and report anything that does not reproduce.
Any increase over about 20 sites, or a new shape family: send me the per-project numbers for a review.

Reports. V = `premise-audit/astra/adversarial.md`. T = `premise-audit/fable/adversarial.md`.
C = `premise-audit/api-census/adversarial.md`. H = `small-proofs/c1/adversarial.md`.
All under `/tmp/sc-scratch.noindex/`. Item names below are unique; the reports reuse D1 and so on, so the letter
says which. Report rulings beat the Fable reports they review.

## 1. False positives (users see these)

1. V-D1: no-op store operations, SC2003. Uncertifiable unless ADR 0491's literal proof holds, which needs a facts
   field. Fable's proposed fix is unsound (`fill(0, 5)`, elisions, inherited `Object.assign` source). Includes
   V's 7 further forms. V-D1b: correct the premise text only.
2. T-D3 and C-D1: SC5001 under `<Dynamic>` is a real false positive (the read suspends and retries). SC1001 there
   is true: keep it, fix the hint. `clientOnly`: quiet only when mounted before the module loads, a later mount
   warns, so it must be uncertifiable and must not get the no-window role. Add the `component` prop to the
   exclusive-mount census.
3. C-D2: SC1001 on a Solid callback in an escaping closure, 19 programs. Cause: `inside_non_component_function`
   (`owners/async_context.rs:1282`) treats a primitive callback at any depth as clearing every enclosing function,
   and `inheriting_call_role` falls to the lexical arm. SC1001 only.
4. H-D4: `__defineGetter__`, `valueOf()` on a closed receiver.
5. T: subclass behind a base type (`const a: number[] = new Late()`, `d05`, `d06`, `g24`). T: component mounted
   from a cleanup (`g10`), a provable small fix.
6. C: the inversion, illegal `d16` uncertifiable while legal `d17` is a violation.

## 2. The flip

7. F0 (H-D1), first, because it shares a predicate with the census: a resolved call drops the net only if the
   target's summary is complete. It owns handed values. Check your S0 against it.
8. The value-use census (C section on the flip): a new census called from `reactive_analysis.rs` right after line
   327, widening `reactive_source_uncaptured` (`shared_reactivity.rs:98`). Its `match` on position defaults to an
   obligation, and that default is the flip. It owns proven sources. The call-side fix is rejected: at least
   +1,788, and it misses 15 false cleans. Exclude `splitProps`/`mergeProps` uses (1,376): they need view rows.
   Estimated cost +60 to +600 sites on 11,179, judged 130 to 300. Measure it; it needs a review. Give each new
   obligation a named `blocker` reason.
9. What the census leaves: `children`/`mapArray`/`repeat` results as accessors with `.toArray()` as a read (T-D2,
   proved violation), async generator role, total `handed_value`, direct `createMemo(fn)()` in a body,
   `await Promise.resolve(c)` then a read.
10. V-D2 store flow: forward flow from proven and handed stores, including `For` rows, `for…of`, `forEach`, `find`
    and memo results, uncertifiable. Measure the handed half first. "Any parameter may be a store" is rejected
    (+3,869). V-D5 props: extend ADR 0630 and `parameter_get_sites` (`merge({a:0}, props)` destructures,
    `helper(props)`). V-D3 what is left, including curried wrappers (F0) and narrow uncertifiable rules for
    `new Function`, `WeakRef` and tagged templates. T: `[1].forEach(setW)` goes with these.
11. C-D4: a sync memo throwing `NotReadyError`: SC1001 must be uncertifiable too, as for a real async memo, and
    SC5001 is the rule. H-D2 `[{ run: n }]` iterated. H-D3 `get().read()`. H-D5 `<Dynamic component={C} run={setN}/>`
    in `<C>` (`props_reactivity.rs:723-733`). C: `f()[0]()` takes the inner call's identity, an invariant 3 bug.

## 3. Proved rows and the rest

12. V-D4 whole-store reads: proved at the coercing expression. V-D6 write spellings: proved rows, including
    `Reflect.defineProperty`. V-D9 uncertifiable when the literal holds a reactive value.
13. V-D7: resolve `export * as` or SC9012; `window` and ambient channels become narrow uncertifiable rules, not a
    premise. V-D8: scan imported packages, add a reflective-getter veto.
14. T-D1 cleanup in the disposer's window: direct case proved, escaped handles uncertifiable, a disposer axis in the
    fuzzer. New shape family. T-D4 thenables and subclasses: tripwire uncertifiable when a project class overrides
    a row method. T: `untrack(c, "x")`. T: a no-`src` iframe's `load` during `append`.

## 4. Fuzzer axes, by C's ranking

Value kind × path (79 programs, 5 rules). Solid callback × closure layer. Mount × claim. Callee form.
Pending-source kind. Add them as you reach the matching items, not at the end.

## After the queue

Tell me, and I will start the reviews. Then the final order on the final commit.

## Added: the flip for every rule (owner)

The owner wants the flip for every rule, not only value flow. A per-rule inventory is running:
`/tmp/sc-scratch.noindex/premise-audit/flip/report.md`, then an Opus review. It may add a shared mechanism, with each
demanded site ending proved or obligated. Build sections 1 and 2 so that mechanism can absorb them. Do not start
section 2's census until I relay the ruling on the shared mechanism; section 1 goes ahead.

## Change (owner): fewer reviews

No review per increase. Do not stop or wait for me on a rise: measure it, put the per-project numbers in the commit message, read every removed finding yourself, and keep going. The hold on section 2's census stays until the per-rule inventory reports.

## Change (owner, latest, overrides the above)

Patterns only: add them to the fuzzer, no checker fixes, keep them visible as known open defects, then finish. The per-rule flip agent builds the flip next, on this agent's head.
