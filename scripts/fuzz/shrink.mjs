// Delta debugging over the construct tree (never over text): given a program and a
// predicate ("this program still shows the finding"), find a smaller program for which
// it still holds.
//
// Provenance. The search loop, its execution budget, its seen-set of canonical
// candidate keys, its validity filter before any run, its sweep schedule (stay in a
// family while it succeeds, advance on failure, repeat until a full sweep makes no
// progress), the "control must stay clean" hook and the complexity vector that orders
// "smaller" are ported and adapted from solidjs/solid PR #3446 "test(signals): add
// experimental semantic fuzzer" (open; commit 030e77f2b, MIT), directory
// packages/signals/tests/semantics/: search.ts (SearchBudget, search), shrink.ts
// (fingerprint-preserving trial, control), complexity.ts (compareSize) and
// candidates.ts (`turnsCandidates`, the halving-width chunk removal used here as
// ddmin's list reduction). Adapted: that fuzzer reduces a flat scenario record of
// numeric nodes, readers and turns; this one reduces a tree of Solid source constructs,
// so the candidate families are generic list/node edits (remove a chunk of any list,
// inline a scope's body, replace an expression by a literal) instead of per-field edits.
// Dropped: delivery-ordering and escape families (engine-specific), the restart
// schedule (kept only the sweep, its default), expensive-family staging.
import { clone, getAt, isValid, nodeCount, num, slotsOf, walk } from "./model.mjs";
import { printProgram } from "./print.mjs";

/** Counts actual predicate executions (a checker run, a runtime run); candidates that fail the cheap filters cost nothing. */
export class SearchBudget {
  constructor(limit, minimum = 1) {
    this.limit = limit;
    this.minimum = minimum;
    this.executions = 0;
    this.candidates = 0;
    this.candidateLimit = Math.max(1000, limit * 100);
  }
  available(count = this.minimum) { return this.executions + count <= this.limit; }
  async run(execute) {
    if (!this.available(1)) throw new Error("Reduction execution budget exceeded");
    this.executions += 1;
    return execute();
  }
}

/** A stable preference for less machinery: [nodes, components, helpers, depth]. */
export function complexity(program) {
  let depth = 0;
  for (const { path } of walk(program)) depth = Math.max(depth, path.length);
  return [nodeCount(program), program.components.length, program.helpers.length, depth];
}
export function compareSize(a, b) {
  const left = complexity(a); const right = complexity(b);
  for (let i = 0; i < left.length; i += 1) if (left[i] !== right[i]) return left[i] - right[i];
  return 0;
}

/** The canonical key of a program: its printed text (names are positional, so equal text is an equal program). */
export const programKey = (program) => printProgram(program).text;

// ---------------------------------------------------------------------------
// Candidate families. Each is a generator over edited clones of the current program.

/** Remove chunks of every list, widest chunks first, halving down to single elements (ddmin's reduction). */
function* listChunks(program) {
  const targets = [];
  for (const { node, path } of walk(program)) for (const [key, isList] of slotsOf(node)) if (isList) targets.push({ path, key, length: node[key].length });
  for (const { path, key, length } of targets) {
    for (let width = Math.max(1, Math.ceil(length / 2)); width >= 1; width = Math.floor(width / 2)) {
      for (let at = 0; at < length; at += width) {
        const copy = clone(program);
        getAt(copy, path)[key].splice(at, width);
        yield copy;
      }
      if (width === 1) break;
    }
  }
}

const INLINABLE = new Set(["untrack", "root", "owner", "timer", "then", "settled", "cleanup"]);
/** Replace a statement that only runs a scope (an IIFE, forEach/map, untrack, createRoot, ...) by that scope's body. */
function* inlineScopes(program) {
  const found = [];
  for (const { node, path } of walk(program)) {
    for (const [key, isList] of slotsOf(node)) {
      if (!isList) continue;
      node[key].forEach((stmt, index) => {
        if (stmt.k !== "expr") return;
        const e = stmt.e;
        const scope = e.k === "iife" || e.k === "iter" ? e.fn : e.k === "prim" && INLINABLE.has(e.name) ? e.fn : null;
        if (!scope || scope.k !== "fn" || scope.params.length) return;
        found.push({ path, key, index, body: scope.body });
      });
    }
  }
  for (const { path, key, index, body } of found) {
    const copy = clone(program);
    // A `return` has no function to return from once inlined; keep its expression as a statement.
    const spliced = clone(body).map((s) => (s.k === "return" ? { k: "expr", e: s.e } : s));
    getAt(copy, path)[key].splice(index, 1, ...spliced);
    yield copy;
  }
}
/** Replace a container by the statements it holds (an `await` and a trailing `return null` go with the container's function). */
function* uncontain(program) {
  const found = [];
  for (const { node, path } of walk(program)) {
    for (const [key, isList] of slotsOf(node)) {
      if (!isList) continue;
      node[key].forEach((stmt, index) => { if (stmt.k === "contain") found.push({ path, key, index }); });
    }
  }
  for (const { path, key, index } of found) {
    const copy = clone(program);
    const list = getAt(copy, path)[key];
    const body = list[index].body.filter((s) => s.k !== "await" && s.k !== "return");
    list.splice(index, 1, ...body);
    yield copy;
  }
}
/** Replace one expression slot by a literal. The site-bearing slots fail the cheap filter and cost nothing. */
function* simplifyExpressions(program) {
  const slots = [];
  for (const { node, path } of walk(program)) {
    for (const [key, isList] of slotsOf(node)) {
      if (isList || !["e", "default", "when"].includes(key)) continue;
      if (node[key].k === "num") continue;
      slots.push({ path, key });
    }
  }
  for (const { path, key } of slots) {
    const copy = clone(program);
    getAt(copy, path)[key] = num(1);
    yield copy;
  }
}

/** Drop the trailing return of a block (the statement stays as an expression). */
function* unreturn(program) {
  for (const { node, path } of walk(program)) {
    for (const [key, isList] of slotsOf(node)) {
      if (!isList) continue;
      const last = node[key].length - 1;
      if (last < 0 || node[key][last].k !== "return") continue;
      const copy = clone(program);
      getAt(copy, path)[key].splice(last, 1, { k: "expr", e: node[key][last].e });
      yield copy;
    }
  }
}

export const families = {
  lists: listChunks,
  inline: inlineScopes,
  uncontain,
  unreturn,
  simplify: simplifyExpressions,
};
export const familyOrder = ["lists", "inline", "uncontain", "unreturn", "simplify"];

// ---------------------------------------------------------------------------

/**
 * Reduce `initial` while `predicate` holds.
 * @param {object} initial the program (it must satisfy the predicate)
 * @param {(program: object) => Promise<boolean>} predicate expensive: runs the pipeline
 * @param {{ budget?: number, cheap?: (program: object) => boolean, control?: (program: object) => Promise<boolean>, order?: string[], onAccept?: (program: object, family: string) => void,
 *   judgeBatch?: (programs: object[]) => Promise<boolean[]>, batchSize?: number }} [options]
 *   `cheap` is a free pre-filter (e.g. "still has a site"); `control` must stay true on every accepted program.
 *   `judgeBatch` judges several candidates in one run (one fast-path process for the lot): it must answer what `predicate` would,
 *   program by program. The search then takes the first candidate, in the order the family yields them, that holds, exactly as the
 *   one-at-a-time loop does; the candidates after it were judged for nothing, so they are neither counted against the budget nor
 *   marked seen, and the result is the sequential one.
 */
export async function shrink(initial, predicate, { budget = 150, cheap = () => true, control, order = familyOrder, onAccept, judgeBatch, batchSize = 8 } = {}) {
  const limit = new SearchBudget(budget);
  let current = initial;
  const seen = new Set([programKey(initial)]);
  let accepted = 0; let duplicates = 0; let invalid = 0; let skipped = 0; let controlAttempts = 0; let rounds = 0;
  let progress = true;
  outer: while (progress && limit.available() && limit.candidates < limit.candidateLimit) {
    progress = false;
    for (const family of order) {
      let again = true;
      while (again && limit.available()) {
        again = false;
        const candidates = families[family](current)[Symbol.iterator]();
        let drained = false;
        while (!drained && !again) {
          // Gather the next candidates that pass the free filters (at most as many as the budget can still pay for).
          const room = judgeBatch ? Math.min(batchSize, limit.limit - limit.executions) : 1;
          if (room < 1) break;
          const batch = [];
          while (batch.length < room) {
            const next = candidates.next();
            if (next.done) { drained = true; break; }
            const candidate = next.value;
            if (limit.candidates >= limit.candidateLimit) break outer;
            limit.candidates += 1;
            if (!isValid(candidate)) { invalid += 1; continue; }
            if (compareSize(candidate, current) >= 0) { skipped += 1; continue; }
            if (!cheap(candidate)) { skipped += 1; continue; }
            const key = programKey(candidate);
            if (seen.has(key)) { duplicates += 1; continue; }
            seen.add(key);
            batch.push({ candidate, key });
          }
          if (!batch.length) continue;
          rounds += 1;
          const verdicts = judgeBatch ? await judgeBatch(batch.map((entry) => entry.candidate)) : [await predicate(batch[0].candidate)];
          let take = -1; let controlRuns = 0;
          for (let index = 0; index < batch.length; index += 1) {
            if (!verdicts[index]) continue;
            if (control) {
              controlAttempts += 1;
              if (limit.executions + index + 1 + controlRuns >= limit.limit) break;
              controlRuns += 1;
              if (!(await control(batch[index].candidate))) continue;
            }
            take = index;
            break;
          }
          // Executions: those a one-at-a-time loop would have run (up to and including the one it takes), and its control runs.
          limit.executions += (take < 0 ? batch.length : take + 1) + controlRuns;
          // The candidates after the one taken were not reached: another round may meet them again.
          for (let index = take < 0 ? batch.length : take + 1; index < batch.length; index += 1) seen.delete(batch[index].key);
          if (take >= 0) {
            current = batch[take].candidate;
            seen.add(batch[take].key);
            accepted += 1;
            onAccept?.(current, family);
            progress = true;
            again = true;
          }
          if (!limit.available()) break outer;
        }
      }
    }
  }
  return {
    program: current, accepted, duplicates, invalid, skipped, controlAttempts, rounds,
    attempts: limit.executions, candidates: limit.candidates,
    exhausted: !limit.available() || limit.candidates >= limit.candidateLimit,
  };
}
