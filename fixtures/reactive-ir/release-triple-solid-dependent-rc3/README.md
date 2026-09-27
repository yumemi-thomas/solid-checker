# release-triple-solid-dependent-rc3

A project with no Solid code of its own and no Solid module named, which
imports a package, `solid-format`, whose manifest peer-depends on
`solid-js`. On an unaudited `2.0.0-rc.3` triple.

What that package's surface answers -- a contract, a typing -- can depend on
the Solid release it was built against, so importing it is not ruled out as a
use of the runtime: the census reads the installed dependency closure of every
package the project reaches, and a closure that names a Solid 2 package keeps
the notice (`rust/crates/solid-facts-backend/src/release_scope.rs`,
`solid_use`). It is the fail-closed direction the census takes throughout: a
package reference whose resolution the facts do not carry, or a closure with a
required dependency that is not installed, keeps it too.

Expected: the `SC9014` notice. The package has no contract, and the call to
`format` is `SC9005` (a package in the Solid ecosystem with no contract), which
this fixture does not own. Asserted by
`the_release_notice_is_due_only_for_a_project_that_uses_solid`
(`rust/crates/solid-facts-backend/tests/dialects_process.rs`).

`node_modules/` holds the three Solid manifests at `2.0.0-rc.3` and
`solid-format`; `tsc --noEmit` (5.9.3, `strict`, bundler resolution) is clean.
