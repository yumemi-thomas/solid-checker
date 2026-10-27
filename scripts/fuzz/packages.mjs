// The `packages` profile: a callback handed to a function of a generated dependency package (package.mjs).
//
// The construct tree is the ordinary one (a payload in layers, a host, a disposition: generate.mjs). What changes is the helper layer:
// its callee is an export of `fz-lib`, one of the behaviours of package.mjs in one of its export forms, so the checker has to read the
// package's source to know whether, when and under which owner the callback runs. The generator knows each behaviour, so every
// program's runtime truth is determined by construction; the fast path and Chrome are the judges all the same.
//
// A read payload is drawn only when every package helper on the chain hands the callback's value back (value, fn and obj results):
// a package that drops the value (a timer, an effect) could not make a read's witness land.
import { BEHAVIOURS, EXPORT_FORMS, formOf, namesOf } from "./package.mjs";
import { call, expr, fn, helper, prim, ref, button } from "./model.mjs";

/** Where the package is called from: the hosts of the brief (body, memo, effect, handler, untrack) and the other scopes a call can sit in. */
export const PACKAGE_HOSTS = { memo: 5, effect: 3, untrack: 2, root: 1, owner: 2, body: 4, apply: 2, settled: 1, cleanup: 0, timer: 1, then: 1, handler: 3, jsx: 1, async: 0 };

const BEHAVIOUR_WEIGHTS = [
  [5, "inline"], [2, "repeat"], [3, "timer"], [2, "micro"], [2, "then"], [3, "untrack"], [3, "ownerNull"], [3, "ownerNow"], [2, "root"],
  [2, "effectCompute"], [2, "effectApply"], [1, "memo"], [2, "cleanup"], [1, "settled"], [2, "never"], [3, "returnCb"], [3, "returnWrap"],
  [3, "storeObj"], [4, "register"],
];
const EXPORT_WEIGHTS = [
  [5, "named"], [3, "arrow"], [2, "default"], [3, "reexport"], [1, "reexportRenamed"], [1, "star"], [2, "class"], [1, "static"], [2, "object"], [1, "namespace"],
];

/** A package behaviour drawn with its export form and options; a plain data spec (`helper.pkg`). */
export function choosePackageSpec(rng) {
  const behaviour = rng.weighted(BEHAVIOUR_WEIGHTS);
  let exportForm = rng.weighted(EXPORT_WEIGHTS);
  // `register` needs a second export for `fire`: not a default, which is a single function.
  if (behaviour === "register" && exportForm === "default") exportForm = "named";
  const spec = { behaviour, exportForm };
  if (BEHAVIOURS[behaviour].direct && rng.chance(0.4)) spec.wrap = true;
  if (behaviour === "repeat") spec.n = rng.pick([2, 3]);
  if (behaviour === "register") {
    spec.store = rng.weighted([[3, "var"], [1, "array"], [1, "object"]]);
    if (rng.chance(0.4)) spec.owned = true;
  }
  return spec;
}

/** 1 to 3 layers; the innermost wraps the payload and is always a package export. */
export function chooseLayersPackages(ctx) {
  const { rng } = ctx;
  const depth = rng.weighted([[5, 1], [3, 2], [1, 3]]);
  const layers = [];
  for (let index = 0; index < depth; index += 1) {
    const kind = index === 0 ? "pkg" : rng.weighted([[5, "pkg"], [3, "closure"], [1, "iife"], [1, "iter"], [1, "default"]]);
    switch (kind) {
      case "pkg": layers.push({ k: "pkg", pkg: choosePackageSpec(rng) }); break;
      case "closure": layers.push({ k: "closure", named: rng.chance(0.6), form: rng.chance(0.2) ? "function" : "arrow", async: false }); break;
      case "iife": layers.push({ k: "iife" }); break;
      case "iter": layers.push({ k: "iter", method: rng.pick(["forEach", "map"]) }); break;
      default: layers.push({ k: "default", named: rng.chance(0.6), destructured: rng.chance(0.35) });
    }
  }
  return layers;
}

/** write, read or owner call; a read only where every package helper returns the callback's value. */
export function packagePayloadKind(rng, layers) {
  const valueOnly = layers.every((layer) => layer.k !== "pkg" || ["value", "fn", "obj"].includes(BEHAVIOURS[layer.pkg.behaviour].result))
    && !layers.some((layer) => layer.k === "iter" && layer.method === "forEach");
  return rng.weighted([[4, "write"], [valueOnly ? 3 : 0, "read"], [2, "own"]]);
}

/**
 * The helper for a package spec (made once per distinct spec); returns its name. A `register` also gets its `fire`,
 * recorded in `ctx.fireOf`, and at most one export of a program is the package's default.
 */
export function ensurePkgHelper(ctx, spec) {
  ctx.fireOf ??= new Map();
  const taken = [...ctx.helpers.values()].some((existing) => existing.pkg?.exportForm === "default");
  const wanted = spec.exportForm === "default" && taken ? { ...spec, exportForm: "named" } : spec;
  const key = JSON.stringify(wanted);
  for (const existing of ctx.helpers.values()) if (existing.pkg && JSON.stringify(existing.pkg.spec ?? null) === key) return existing.name;
  const make = (pkg) => {
    const added = { ...helper(`h${ctx.helpers.size + 1}`, formOf(pkg.behaviour, pkg.exportForm)), pkg };
    ctx.helpers.set(added.name, added);
    return added;
  };
  const registered = make({ ...wanted, spec: wanted });
  if (wanted.behaviour === "register") {
    const container = namesOf(registered).container;
    if (container) registered.pkg.container = container;
    const fired = make({ behaviour: "fire", exportForm: wanted.exportForm, of: registered.name, ...(container ? { container } : {}) });
    ctx.fireOf.set(registered.name, fired.name);
  }
  return registered.name;
}

/** Where the `fire` call goes: a click (the callback runs later, from a handler), the component body, or a timer. */
export function firePlacement(ctx, fireName, { body, view, name }) {
  const where = ctx.plan?.fire ?? ctx.rng.weighted([[13, "handler"], [4, "body"], [3, "timer"]]);
  const fire = () => expr(call(ref(fireName)));
  if (where === "handler") view.push(button(name("b"), [fire()]));
  else if (where === "body") body.push(fire());
  else body.push(expr(prim("timer", fn([fire()]))));
}

export { EXPORT_FORMS };
