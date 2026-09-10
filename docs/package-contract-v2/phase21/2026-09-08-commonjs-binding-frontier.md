# CommonJS export identity frontier

The report-bound candidates are `@solidjs/testing-library@0.8.10` through
`aria-query@5.3.0`, and `@tanstack/ai-solid@0.19.1` through
`partial-json@0.1.7`, both Solid 1 probes in
`2026-09-08-unique-read-origin-full.json`. Each candidate declares one
entrypoint. Two complete rows are an unmeasured ceiling, not a gain;
downstream AI UI remains a separate, unmeasured dependency opportunity.

The retained packages need more than direct export assignments. Partial JSON
uses local `require`, getter exports and `__exportStar`/`__createBinding`;
Aria Query uses local requires, a default-interop helper and final assignments
to the exports object. The existing generator's runtime export census handles
ESM and correctly refuses these surfaces. Type Facts already has compiler
module exports, but compiler symbol availability is not CommonJS runtime
export authority.

Two local controls ran under the exact installed Node v24.11.1 executable,
SHA-256 `4255a388254ca4319e2f95f1da375d5deaddf25baf9c7c85070b67f9543b15d0`.
The command `node /private/tmp/commonjs-binding-controls/check.mjs` exits 0.
Its source hashes and observations are retained in that directory's
`result.json`. No package acquisition or production-source change was used.

```js
function original() { return "original"; }
exports.run = original;
exports.replace = function replace() {
  exports.run = () => "replacement";
};
```

After invoking `replace`, the named ESM `run` still returns `original`,
whereas the default import's `run` and `require` result's `run` both return
`replacement`. A separate module containing the following statements exposes
a named namespace key `run` with value `undefined`; its default object has
no `run` property:

```js
module.exports = {};
exports.run = function orphaned() { return "orphaned"; };
```

This agrees with the version-specific
[Node CommonJS namespace documentation](https://nodejs.org/download/release/v24.11.1/docs/api/esm.html#commonjs-namespaces):
named exports are copied from the initialized CommonJS export value, and
subsequent updates do not change those copies. Detected names alone do not
prove initialized values. These controls establish distinctions a future
proof must preserve; runtime observations alone authorize no receipt.

A bounded implementation must bind the host/module interpretation, exact
import-versus-require selection, final export object identity, initialization
order, stable named value origins, and exact local dependency receipts.
Reassignment of the wrapper's `exports`/`module.exports`, overwritten exported
properties, getters, cycles, escaped export objects, and unknown initialization
effects must either have positive proof or remain refused. Treating CommonJS
as ESM, copying compiler export names, or sampling a namespace is insufficient.
The first semantic slice should prove direct final function assignments in
an isolated static module and include these controls before supporting either
published target's interop/getter composition. A narrow proof slice does not
itself recover the targets until their full required cases certify.

No new accepted cases, complete rows, or metric corrections are claimed here.
The independently completed helper-proof full corpus is recorded in ADR 0087;
these CommonJS controls do not contribute any of its accepted cases.

The local Type Facts compiler was also queried through a temporary Go test
overlay, without changing production code or measurement binaries. It lists
`run` and `replace` for the first control, but lists no exports for the rebound
control. Node's namespace still contains the rebound control's `run` key with
value `undefined`. Thus compiler module exports and the host's detected named
namespace are distinct censuses, not interchangeable authority. Both overlay
observations pass in 0.047 seconds; source and log are retained under
`/private/tmp/commonjs-binding-controls/` (`overlay.json`,
`compiler_exports_test.go`, and `compiler-observation.log`). The result is
not evidence that an empty compiler export list means no runtime surface.

The same overlay queried the exact retained runtime files of both published
dependencies, and Node imported those files successfully. Partial JSON's
runtime entry digest is
`sha256:23f750aa1170c830b7ef180135852317886cc58ce7d2597e5eeee044539072c7`;
Aria Query's is
`sha256:8c77c44ed7e2dfa1b8d2b94b2bc418f9a3a25b6c2cb7ec844da562d6b6c88a6d`.
Node observes the parser functions and numeric option exports for Partial
JSON, and the five object exports for Aria Query. This establishes a runnable
candidate surface under that host, not contract behavior or graph acceptance.

The compiler marks each principal published export with two declarations;
its primary value declaration points to the initial chained `void 0`
assignment, not the final function or map assignment. Partial JSON's compiler
list also omits the numeric reexports visible in Node's namespace. A proof
must therefore establish the final initialized value separately and compose
reexports through exact local modules; neither the primary value declaration
nor the compiler export list can be promoted wholesale to runtime authority.
Both published compiler observations pass in 0.047 seconds. The overlay,
source and logs are `published-overlay.json`, `published_exports_test.go`,
`published-compiler-observation.log`, `published.mjs`, and
`published-node-observation.json` in the same temporary control directory.

A further fresh-process counterexample shows that even a direct final
function assignment needs an initialization premise. Before importing the
unchanged `run.cjs`, install a configurable setter for `run` on
`Object.prototype`. It intercepts `exports.run = original`; Node then exposes
the named `run` with value `undefined` and the default export has no own `run`.
The control restores the original prototype in `finally`, checks the
interception and both observations, and exits 0 under the same pinned Node.
See `prototype-setter.mjs` and `prototype-setter-result.json` in the control
directory. This does not make the package intrinsically broken; it shows
which host state an unconditional function-binding proof would overlook.

Accordingly, the first direct-assignment slice above must bind the export
object's applicable property semantics at initialization, rather than assume
a pristine prototype or infer an own data property from assignment syntax.
The existing Type Facts `ModuleFormat` is also explicitly the compiler's
**emit** format; under a configured non-Node module kind it cannot by itself
authenticate how Node loads the published bytes. Loader interpretation and
initialization premises need their own positive, consumer-bound evidence.
