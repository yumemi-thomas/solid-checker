# One real development-artifact reads pilot

The pilot certified `noop`'s empty reads closure in
`@solid-primitives/utils@7.0.0-next.4`, selected with `import,browser,development`.
Ordinary analysis authenticated the issued receipt and selected its exact case.
The resulting package had one reads closure, 25 creates closures and three
return closures. Only the reads recipe was added for this experiment.

The package installation was assembled in an isolated temporary project from
existing files. Utils came from its retained published installation; Solid,
web and signals came from the audited `2.0.0-rc.3` oracle installation, with
its existing exact Bun lock entries and supporting dependencies. This avoided
the newer signals prerelease in the retained ecosystem installation. No package
was downloaded and no checked-in installation was modified. The certification
transaction independently acquired the published archives from the authenticated
cache; a cache miss would fail rather than use the network.

The scratch-only recipe invokes the exact `noop` export without supplying
callbacks, accessors, or objects. It executes once in a fresh tracking compute
scope and observes `DEV.getSources(getObserver())` in the exact development
runtime. An observed source emits `read-operation`. Missing observer capability
or an incomplete invocation throws rather than producing a clean observation.
The implementation census supplies the independent proof of the reads closure.

Three fresh Node processes exercised the same observation function under the
development conditions:

| Invocation | Contradiction markers |
| --- | ---: |
| Actual published `noop` | 0 |
| Separate control reading a module-private signal, discarding its value | 1 |
| Separate control reading that signal through `untrack` | 0 |

The controls are observation checks, not authenticated certification of the
control package, and were not run through the private certification worker.
The real package certification did use that worker, its authenticated copies,
mandatory gate, and receipt binding. The earlier 16-scenario worker experiment
also remains available in `2026-09-12-reads-observation-experiment.md`.

This is **one development-artifact closure**, not a production-corpus increase.
The observer misses untracked and deferred reads, nested observers, other
runtimes, and unvisited branches. It does not infer authorship for arbitrary
caller callbacks or objects. Those limitations prevent promoting it as a
general reads synthesizer. The recipe remains in the temporary pilot corpus;
no production recipe or proof policy was changed.

The [measurement](2026-09-12-self-import-and-reads-pilot-measurement.json)
retains the public result, recipe source, controls, package versions, and hashes.
The temporary project is `/private/tmp/solid-reads-pilot-fvflxjti`; its
`recipes/recipes.json` adds only the exact `noop` reads claim to a copy of the
existing corpus. The reusable offline runner accepts `--conditions`,
`--entrypoint`, and `--probe-recipe-corpus` to reproduce this attempt while that
project and its authenticated cache entries remain available.
