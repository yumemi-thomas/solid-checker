// What scripts/repro-reduce.mjs keeps while it deletes lines (ADR 0956): the result it was asked to hold, as a key, and whether a checker finding is that result.
//
// Two keys. The text key is the original: the rule, the message family and the analysis context kind (scripts/uncertifiable-causes.mjs), on a line
// containing a marker. Those are labels read from wording, so a reduction can drift to a different cause behind the same words. The typed key
// names the blocker the checker itself reports (`blockedBy`, ADR 0947): its `kind`, and for a veto the `formula` and the exact set of hazards
// that `fired`, or for a row decided by one test the `missing` fact. The reduction then keeps that blocker, or it stops: a program that keeps the
// same (formula, fired set) is the same veto, whatever its text says.
import { contextKind, messageFamily } from "../uncertifiable-causes.mjs";

/** The key `options` ask for, or throws on a mix or a gap. `options` carries the CLI values (strings), absent ones null or "". */
export function parseKey(options) {
  const typed = Boolean(options.blocker);
  if (!typed) {
    for (const name of ["formula", "fired", "missing"]) if (options[name]) throw new Error(`--${name} needs --blocker: it names part of a typed blocker`);
    for (const name of ["rule", "family", "marker"]) if (!options[name]) throw new Error(`--${name} is required (or name the typed blocker with --blocker)`);
    return { kind: "text", rule: options.rule, family: options.family, context: options.context ?? "", marker: options.marker };
  }
  for (const name of ["family", "context"]) if (options[name]) throw new Error(`--${name} belongs to the text key; with --blocker the key is typed`);
  if (options.fired && !options.formula) throw new Error("--fired needs --formula: it is the set the formula's row fired");
  if (options.formula && !options.fired) throw new Error("--formula needs --fired: the set of hazards that fired is part of the key");
  if (options.formula && options.missing) throw new Error("--formula and --missing are two kinds of blocker: name one");
  const fired = String(options.fired ?? "").split(",").map((atom) => atom.trim()).filter(Boolean).sort();
  let missing = null;
  if (options.missing) {
    const at = String(options.missing).indexOf(":");
    missing = at < 0 ? { fact: options.missing, member: null } : { fact: options.missing.slice(0, at), member: options.missing.slice(at + 1) };
    if (!missing.fact) throw new Error("--missing names a fact: <fact> or <fact>:<member>");
  }
  return { kind: "typed", blocker: options.blocker, formula: options.formula || null, fired: options.formula ? fired : null, missing, rule: options.rule || null, marker: options.marker || null };
}

/** The key as one line, for the logs. */
export function describeKey(key) {
  if (key.kind === "text") return `${key.rule} ${key.family}${key.context ? ` ${key.context}` : ""} on a line with ${JSON.stringify(key.marker)}`;
  const part = [key.blocker];
  if (key.formula) part.push(`${key.formula} [${key.fired.join(", ")}]`);
  if (key.missing) part.push(`missing ${key.missing.fact}${key.missing.member ? `:${key.missing.member}` : ""}`);
  if (key.rule) part.push(key.rule);
  if (key.marker) part.push(`on a line with ${JSON.stringify(key.marker)}`);
  return part.join(" ");
}

const sameSet = (left, right) => left.length === right.length && [...left].sort().every((atom, index) => atom === [...right].sort()[index]);

/**
 * Whether `finding` (one of the checker's JSON findings) is the result `key` holds, in `target` whose lines are `lines`. An uncertifiable
 * finding only; it must stand in the target file.
 */
export function holdsKey(key, finding, target, lines) {
  if (finding.kind !== "uncertifiable" || finding.primaryLocation?.path !== target) return false;
  const text = lines[finding.primaryLocation.line - 1] ?? "";
  if (key.kind === "text") {
    return finding.id === key.rule && messageFamily(finding.message) === key.family && contextKind(finding.analysisContext) === key.context && text.includes(key.marker);
  }
  const blocked = finding.blockedBy;
  if (!blocked || blocked.kind !== key.blocker) return false;
  if (key.rule && finding.id !== key.rule) return false;
  if (key.marker && !text.includes(key.marker)) return false;
  if (key.formula && (blocked.formula !== key.formula || !sameSet(blocked.fired ?? [], key.fired))) return false;
  if (key.missing && (blocked.missing?.fact !== key.missing.fact || (key.missing.member !== null && (blocked.missing.member ?? null) !== key.missing.member))) return false;
  // A key with neither a formula nor a missing fact is the kind alone: any row of that kind holds it.
  return true;
}
