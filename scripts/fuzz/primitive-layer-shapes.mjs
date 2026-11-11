// The primitive-layer shapes (ADR 0940): the site in the callback of a primitive that runs it in place, nested in a host.
//
// The checker reads a leaf owner, a tracked scope or an action step from the primitive that opens it, and the callbacks between that primitive and
// the site were each a place nobody had measured: `untrack(() => onCleanup(…))` in `createTrackedEffect` is still a leaf owner's child (untrack clears
// the observer, never the owner), `latest(() => resolve(…))` still runs under the tracking observer, `isPending(fn)` catches what `fn` throws, `createRoot`
// under a leaf owner is the forbidden primitive itself, `runWithOwner(owner, …)` goes back to the host's own owner and `runWithOwner(null, …)` leaves
// it. Each layer changes which of the ten codes the runtime reports at the site, so each pair (layer, host) is a context of its own, crossed with every
// code by the census.
//
//   pl-<layer>-<host>       the site is the layer's callback body, the layer is a statement of the host's callback
//
//   layers   untrack, latest, ispending (`isPending`), owner (`runWithOwner(owner, …)`, the owner the host's callback captured first),
//            owner-null (`runWithOwner(null, …)`), root (`createRoot`), flush (`flush(fn)`: rc.21 runs `fn` at once, then drains)
//   hosts    tracked (`createTrackedEffect`), settled (an `onSettled` the body's owner backs), effect-apply (an effect's apply), memo-async (an async
//            `createMemo` after its `await`; the `owner` layer captures the owner before the `await`), action (an action's first step, called from a
//            button), body (the component's body)
//
// What the runtime reports in each cell is MEASURED, not derived (`PL_MEASURED`, filled by scripts/fuzz/primitive-layer-scan.mjs from Chrome): a cell is
// a misuse cell of a code where Chrome reports the code at the site, a legal cell where it reports no defect code and the site ran, and neither where
// another code comes first (`createRoot` under a leaf owner throws at the `createRoot` call, the site never runs). A pair that is neither is still judged,
// by the census, against every code, as any context is.
//
// Recall contexts only, base shapes (misuse.mjs BASE_ONLY_CONTEXTS, `fireShape`): the question is the layer between the host and the site, not the
// spelling of the site.
import { awaitStmt, bareParam, button, call, decl, expr, flush, fn, getOwner, num, prim, ref, ret } from "./model.mjs";

/** The layers, in the order of the context names. `owner-null` is listed before `owner`: the parse below tries the longer name first. */
export const PL_BASE_LAYERS = ["untrack", "latest", "ispending", "owner", "owner-null", "root", "flush"];
/**
 * ADR 0960: the callback a layer hands the site to, in the forms the slot climbs ended at. `param`: a callback that takes an argument it never uses
 * (`untrack((p0?: number) => …)`; `createRoot`'s own parameter is its dispose); `async`: an async callback whose site runs before its first await
 * (`untrack(async () => { … })`). Each is a layer value of its own, `pl-<layer>-<form>-<host>`, crossed with every host.
 */
export const PL_CALLBACK_FORMS = ["param", "async"];
export const PL_LAYERS = [...PL_BASE_LAYERS, ...PL_BASE_LAYERS.flatMap((layer) => PL_CALLBACK_FORMS.map((form) => `${layer}-${form}`))];
/** `{ base, form }` of a layer name: `owner-null-async` is the `owner-null` layer in the `async` callback form. */
export const plLayerParts = (layer) => {
  const form = PL_CALLBACK_FORMS.find((one) => layer.endsWith(`-${one}`));
  return form ? { base: layer.slice(0, -(form.length + 1)), form } : { base: layer, form: null };
};
export const PL_HOSTS = ["tracked", "settled", "effect-apply", "memo-async", "action", "body"];
export const PL_CONTEXT_NAMES = PL_HOSTS.flatMap((host) => PL_LAYERS.map((layer) => `pl-${layer}-${host}`));

/**
 * The ten codes whose report depends on the owner or the observer the site runs under, which a primitive layer changes: the three leaf-owner codes,
 * the effect-callback primitive, the two tracked-scope awaits, the three flush codes and a read after an `await`. The code's own template supplies the
 * site (misuse.mjs SPEC); no other code is left out of the layers, the census crosses them with all of them.
 */
export const PL_CODES = [
  "CLEANUP_IN_FORBIDDEN_SCOPE", "PRIMITIVE_IN_FORBIDDEN_SCOPE", "FLUSH_IN_TRACKED_QUEUE_CALLBACK", "PRIMITIVE_IN_EFFECT_CALLBACK",
  "RESOLVE_IN_REACTIVE_SCOPE", "UNTIL_IN_REACTIVE_SCOPE", "FLUSH_IN_ACTION", "FLUSH_IN_EFFECT_CALLBACK", "UNTRACKED_READ_AFTER_AWAIT", "NO_OWNER_CLEANUP",
];

const NAME = new RegExp(`^pl-(${[...PL_LAYERS].sort((a, b) => b.length - a.length).join("|")})-(${PL_HOSTS.join("|")})$`);
/** `{ layer, host }` of a `pl-*` context name, or null. */
export const plParts = (context) => { const match = NAME.exec(context); return match ? { layer: match[1], host: match[2] } : null; };

/**
 * `{ pre, inner }` of a layer around `stmts`: `inner` is the layer's statement, `pre` what the host's callback holds first (the `owner` layer's
 * captured owner: `getOwner()` in a callback is that callback's own owner, so it is read before an `await` the host may hold).
 */
function layered(layer, stmts) {
  const { base, form } = plLayerParts(layer);
  // ADR 0960: the callback the site is the body of. `createRoot`'s callback already takes its dispose, so its `param` form names it.
  const callback = form === "param" ? fn(stmts, { params: [base === "root" ? bareParam("() => void", false) : bareParam()] }) : form === "async" ? fn(stmts, { async: true }) : fn(stmts);
  switch (base) {
    case "untrack": return { pre: [], inner: [expr(prim("untrack", callback))] };
    case "latest": return { pre: [], inner: [expr(prim("latest", callback))] };
    case "ispending": return { pre: [], inner: [expr(prim("isPending", callback))] };
    case "owner": return { pre: [decl("owner", getOwner())], inner: [expr(prim("owner", callback, { ownerArg: "owner" }))] };
    case "owner-null": return { pre: [], inner: [expr(prim("owner", callback, { ownerArg: "null" }))] };
    case "root": return { pre: [], inner: [expr(prim("root", callback))] };
    case "flush": return { pre: [], inner: [expr(flush(callback))] };
    default: throw new Error(`no primitive layer ${layer}`);
  }
}

/** The placement of `stmts` for a `pl-*` context (`{ body, view }`), or undefined for any other. */
export function plContext(context, stmts, kit) {
  const parts = plParts(context);
  if (!parts) return undefined;
  const { pre, inner } = layered(parts.layer, stmts);
  const held = [...pre, ...inner];
  switch (parts.host) {
    case "tracked": return { body: [expr(prim("tracked", fn(held)))], view: [] };
    case "settled": return { body: [expr(prim("settled", fn(held)))], view: [] };
    case "effect-apply": return { body: [expr(prim("effect", fn([ret(num(1))]), { apply: fn(held) }))], view: [] };
    case "memo-async": {
      const m = kit.name("m");
      return { body: [decl(m, prim("memo", fn([...pre, awaitStmt(), ...inner, ret(num(1))], { async: true }))), expr(prim("effect", fn([ret(call(ref(m)))]), { apply: fn([]) }))], view: [] };
    }
    case "action": {
      const a = kit.name("a");
      return { body: [decl(a, prim("action", fn(held)))], view: [button(kit.name("b"), [expr(call(ref(a)))])] };
    }
    case "body": return { body: held, view: [] };
    default: throw new Error(`no primitive-layer host ${parts.host}`);
  }
}

/** The hosts a code takes where it takes fewer than all: a read after an `await` needs the one host that awaits (in the others it is no read after an await). */
export const PL_HOSTS_OF = { UNTRACKED_READ_AFTER_AWAIT: ["memo-async"] };

/**
 * What Chrome (rc.21) reports in each cell, per code, MEASURED (2026-11-10, the build and install of this commit's tree: 840 programs through the Chrome host and
 * the real oracle, 0 behaviour differences between them): `misuse` hosts list the layers whose site gets the code reported, `legal` hosts those where the runtime
 * reports no defect code at all and the site ran. `without` lists the variants a legal cell does not take: the `effect` variant of the two primitive codes reports
 * NO_OWNER_EFFECT wherever the site has no owner (the layer left it, or the host never had one). A layer in neither list is not a cell of that code: another code
 * is reported first, the site never runs (`createRoot` under a leaf owner throws at the `createRoot` call) or the throw is replaced by a later one (a `flush(fn)`
 * drain's own throw hides what `fn` threw). Those pairs are not judged: the census skips the `pl-*` contexts (matrix.mjs `CENSUS_SKIPS`).
 *
 * What the table says about the layers (each row of it is a program Chrome ran):
 *   untrack, latest, isPending, flush(fn), runWithOwner(owner, …)   keep the host's owner: a leaf host still refuses `onCleanup` and a primitive in all of them
 *   runWithOwner(null, …)                                           leaves it: no owner at all (NO_OWNER_*), no leaf, nothing to refuse
 *   createRoot                                                      is the primitive: under a leaf owner it throws before the site runs; elsewhere the site has a root's owner
 *   isPending(fn)                                                   catches what `fn` throws, so a code Solid only throws (`resolve`, `flush` in an action or a tracked queue)
 *                                                                   is not reported under it: the legal cells of those codes (a diagnostic Solid emits before it throws, as
 *                                                                   `onCleanup` in a leaf does, is still reported)
 *   latest(fn)                                                      runs `fn` with an observer, so `resolve` under it in a tracked effect throws where `untrack` and `isPending` do not
 *   UNTRACKED_READ_AFTER_AWAIT takes the one host with an `await` (`memo-async`); in the others the read is no read after an await.
 * @type {Record<string, { misuse: Record<string, string[]>, legal: Record<string, string[]>, without?: Record<string, Record<string, string[]>> }>}
 */
export const PL_MEASURED = {
  CLEANUP_IN_FORBIDDEN_SCOPE: {
    misuse: {
      "tracked": ["untrack", "latest", "ispending", "owner", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "flush-param", "flush-async"],
      "settled": ["untrack", "latest", "ispending", "owner", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "flush-param", "flush-async"],
    },
    legal: {
      "memo-async": ["owner", "root", "owner-param", "owner-async", "root-param", "root-async"],
      "action": ["root", "root-param", "root-async"],
      "body": ["untrack", "latest", "ispending", "owner", "root", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "root-param", "root-async", "flush-param", "flush-async"],
    },
  },
  PRIMITIVE_IN_FORBIDDEN_SCOPE: {
    misuse: {
      "tracked": ["untrack", "latest", "ispending", "owner", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "flush-param", "flush-async"],
      "settled": ["untrack", "latest", "ispending", "owner", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "flush-param", "flush-async"],
    },
    legal: {
      "tracked": ["owner-null", "owner-null-param", "owner-null-async"],
      "settled": ["owner-null", "owner-null-param", "owner-null-async"],
      "memo-async": ["untrack", "latest", "ispending", "owner", "owner-null", "root", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "root-param", "root-async", "flush-param", "flush-async"],
      "action": ["untrack", "latest", "ispending", "owner", "owner-null", "root", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "root-param", "root-async"],
      "body": ["untrack", "latest", "ispending", "owner", "owner-null", "root", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "root-param", "root-async", "flush-param", "flush-async"],
    },
    without: {
      "tracked": {"latest":["resolve","until"],"ispending":["resolve","until"],"owner-null":["effect"],"flush":["resolve","until"],"latest-param":["resolve","until"],"latest-async":["resolve","until"],"ispending-param":["resolve","until"],"ispending-async":["resolve","until"],"owner-null-param":["effect"],"owner-null-async":["effect"],"flush-param":["resolve","until"],"flush-async":["resolve","until"]},
      "settled": {"owner-null":["effect"],"owner-null-param":["effect"],"owner-null-async":["effect"]},
      "memo-async": {"untrack":["effect"],"latest":["effect"],"ispending":["effect"],"owner-null":["effect"],"flush":["effect"],"untrack-param":["effect"],"untrack-async":["effect"],"latest-param":["effect"],"latest-async":["effect"],"ispending-param":["effect"],"ispending-async":["effect"],"owner-null-param":["effect"],"owner-null-async":["effect"],"flush-param":["effect"],"flush-async":["effect"]},
      "action": {"untrack":["effect"],"latest":["effect"],"ispending":["effect"],"owner":["effect"],"owner-null":["effect"],"untrack-param":["effect"],"untrack-async":["effect"],"latest-param":["effect"],"latest-async":["effect"],"ispending-param":["effect"],"ispending-async":["effect"],"owner-param":["effect"],"owner-async":["effect"],"owner-null-param":["effect"],"owner-null-async":["effect"]},
      "body": {"owner-null":["effect"],"owner-null-param":["effect"],"owner-null-async":["effect"]},
    },
  },
  FLUSH_IN_TRACKED_QUEUE_CALLBACK: {
    misuse: {
      "tracked": ["untrack", "latest", "owner", "owner-null", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "flush-async"],
      "settled": ["untrack", "latest", "owner", "owner-null", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "flush-async"],
    },
    legal: {
      "tracked": ["ispending", "ispending-param"],
      "settled": ["ispending", "ispending-param"],
      "memo-async": ["untrack", "latest", "ispending", "owner", "owner-null", "root", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "root-param", "root-async", "flush-param", "flush-async"],
      "action": ["ispending", "ispending-param"],
      "body": ["untrack", "latest", "ispending", "owner", "owner-null", "root", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "root-param", "root-async", "flush-param", "flush-async"],
    },
  },
  PRIMITIVE_IN_EFFECT_CALLBACK: {
    misuse: {
      "effect-apply": ["untrack", "latest", "ispending", "owner", "owner-null", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "flush-param", "flush-async"],
    },
    legal: {
      "tracked": ["owner-null", "owner-null-param", "owner-null-async"],
      "settled": ["owner-null", "owner-null-param", "owner-null-async"],
      "memo-async": ["untrack", "latest", "ispending", "owner", "owner-null", "root", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "root-param", "root-async", "flush-param", "flush-async"],
      "action": ["untrack", "latest", "ispending", "owner", "owner-null", "root", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "root-param", "root-async"],
      "body": ["untrack", "latest", "ispending", "owner", "owner-null", "root", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "root-param", "root-async", "flush-param", "flush-async"],
    },
    without: {
      "tracked": {"owner-null":["effect"],"owner-null-param":["effect"],"owner-null-async":["effect"]},
      "settled": {"owner-null":["effect"],"owner-null-param":["effect"],"owner-null-async":["effect"]},
      "memo-async": {"untrack":["effect"],"latest":["effect"],"ispending":["effect"],"owner-null":["effect"],"flush":["effect"],"untrack-param":["effect"],"untrack-async":["effect"],"latest-param":["effect"],"latest-async":["effect"],"ispending-param":["effect"],"ispending-async":["effect"],"owner-null-param":["effect"],"owner-null-async":["effect"],"flush-param":["effect"],"flush-async":["effect"]},
      "action": {"untrack":["effect"],"latest":["effect"],"ispending":["effect"],"owner":["effect"],"owner-null":["effect"],"untrack-param":["effect"],"untrack-async":["effect"],"latest-param":["effect"],"latest-async":["effect"],"ispending-param":["effect"],"ispending-async":["effect"],"owner-param":["effect"],"owner-async":["effect"],"owner-null-param":["effect"],"owner-null-async":["effect"]},
      "body": {"owner-null":["effect"],"owner-null-param":["effect"],"owner-null-async":["effect"]},
    },
  },
  RESOLVE_IN_REACTIVE_SCOPE: {
    misuse: {
      "tracked": ["latest", "latest-param", "latest-async", "ispending-async", "flush-async"],
    },
    legal: {
      "tracked": ["ispending", "owner-null", "ispending-param", "owner-null-param", "owner-null-async"],
      "settled": ["owner-null", "owner-null-param", "owner-null-async"],
      "memo-async": ["untrack", "latest", "ispending", "owner", "owner-null", "root", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "root-param", "root-async", "flush-param", "flush-async"],
      "action": ["untrack", "latest", "ispending", "owner", "owner-null", "root", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "root-param", "root-async"],
      "body": ["untrack", "latest", "ispending", "owner", "owner-null", "root", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "root-param", "root-async", "flush-param", "flush-async"],
    },
  },
  UNTIL_IN_REACTIVE_SCOPE: {
    misuse: {
      "tracked": ["latest", "latest-param", "latest-async", "ispending-async", "flush-async"],
    },
    legal: {
      "tracked": ["ispending", "owner-null", "ispending-param", "owner-null-param", "owner-null-async"],
      "settled": ["owner-null", "owner-null-param", "owner-null-async"],
      "memo-async": ["untrack", "latest", "ispending", "owner", "owner-null", "root", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "root-param", "root-async", "flush-param", "flush-async"],
      "action": ["untrack", "latest", "ispending", "owner", "owner-null", "root", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "root-param", "root-async"],
      "body": ["untrack", "latest", "ispending", "owner", "owner-null", "root", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "root-param", "root-async", "flush-param", "flush-async"],
    },
  },
  FLUSH_IN_ACTION: {
    misuse: {
      "action": ["untrack", "latest", "owner", "owner-null", "root", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "root-param", "root-async"],
    },
    legal: {
      "tracked": ["ispending", "ispending-param"],
      "settled": ["ispending", "ispending-param"],
      "memo-async": ["untrack", "latest", "ispending", "owner", "owner-null", "root", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "root-param", "root-async", "flush-param", "flush-async"],
      "action": ["ispending", "ispending-param"],
      "body": ["untrack", "latest", "ispending", "owner", "owner-null", "root", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "root-param", "root-async", "flush-param", "flush-async"],
    },
  },
  FLUSH_IN_EFFECT_CALLBACK: {
    misuse: {
      "effect-apply": ["untrack", "latest", "ispending", "owner", "owner-null", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "flush-param", "flush-async"],
    },
    legal: {
      "tracked": ["ispending", "ispending-param"],
      "settled": ["ispending", "ispending-param"],
      "memo-async": ["untrack", "latest", "ispending", "owner", "owner-null", "root", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "root-param", "root-async", "flush-param", "flush-async"],
      "action": ["ispending", "ispending-param"],
      "body": ["untrack", "latest", "ispending", "owner", "owner-null", "root", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "root-param", "root-async", "flush-param", "flush-async"],
    },
  },
  UNTRACKED_READ_AFTER_AWAIT: {
    misuse: {
      "memo-async": ["latest", "ispending", "owner-null", "flush", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-null-param", "owner-null-async", "flush-param", "flush-async"],
    },
    legal: {
      "memo-async": ["untrack", "owner", "root", "untrack-param", "untrack-async", "owner-param", "owner-async", "root-param", "root-async"],
    },
  },
  NO_OWNER_CLEANUP: {
    misuse: {
      "tracked": ["owner-null", "owner-null-param", "owner-null-async"],
      "settled": ["owner-null", "owner-null-param", "owner-null-async"],
      "effect-apply": ["untrack", "latest", "ispending", "owner", "owner-null", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async", "flush-param", "flush-async"],
      "memo-async": ["untrack", "latest", "ispending", "owner-null", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-null-param", "owner-null-async", "flush-param", "flush-async"],
      "action": ["untrack", "latest", "ispending", "owner", "owner-null", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "owner-null-param", "owner-null-async"],
      "body": ["owner-null", "owner-null-param", "owner-null-async"],
    },
    legal: {
      "memo-async": ["owner", "root", "owner-param", "owner-async", "root-param", "root-async"],
      "action": ["root", "root-param", "root-async"],
      "body": ["untrack", "latest", "ispending", "owner", "root", "flush", "untrack-param", "untrack-async", "latest-param", "latest-async", "ispending-param", "ispending-async", "owner-param", "owner-async", "root-param", "root-async", "flush-param", "flush-async"],
    },
  },
};

/**
 * `SOLID_CHECKER_FUZZ_PL_PROBE=1` (primitive-layer-scan.mjs, which re-measures `PL_MEASURED`): every `pl-*` context is a cell of every code of `PL_CODES`, on both
 * sides, and no variant is left out, so one run says what Chrome reports in each pair. Never set for a grid whose rows are judged.
 */
const PROBE = process.env.SOLID_CHECKER_FUZZ_PL_PROBE === "1";

/** The `pl-*` context names of one kind (`misuse` or `legal`) of a code. */
const contextsOf = (code, kind) => (PROBE ? (PL_CODES.includes(code) ? [...PL_CONTEXT_NAMES] : []) : Object.entries(PL_MEASURED[code]?.[kind] ?? {}).flatMap(([host, layers]) => layers.map((layer) => `pl-${layer}-${host}`)));

/** `{ [code]: { misuse, legal } }` for SPEC. */
export function plPlacement() {
  return Object.fromEntries((PROBE ? PL_CODES : Object.keys(PL_MEASURED)).map((code) => [code, { misuse: contextsOf(code, "misuse"), legal: contextsOf(code, "legal") }]));
}

/** `{ [code]: { [context]: variants } }`: the variants a legal cell does not take (SPEC `recallVariantsNotIn`). */
export function plNotVariants() {
  if (PROBE) return {};
  return Object.fromEntries(Object.entries(PL_MEASURED).filter(([, table]) => table.without).map(([code, table]) => [code, Object.fromEntries(Object.entries(table.without).flatMap(([host, byLayer]) => Object.entries(byLayer).map(([layer, variants]) => [`pl-${layer}-${host}`, variants])))]));
}
