# A query-suffixed specifier is an asset import, not a module import

`index.js` imports the shipped sibling `./notify.js` twice: once as a module and
once as `./notify.js?raw`. This is the shape that refused fifty ecosystem
artifact cases -- `@kobalte/solidbase` does
`import content from "./read-preferred-language-cookie.js?raw"` -- because the
resolver treated `?raw` as part of the filename and reported the shipped file as
a missing local closure module.

A bundler's resource query makes the import bundler-mediated. The binding's
value is whatever the loader produces (`?raw` a string, `?url` a URL string,
`?worker` a constructor) and never the target module's exports, so the checker
must not strip the query and walk into the module: that would attribute
`notify`'s exact same-stack callback to a binding that never calls anything.
Nor may it refuse: the file is shipped, and the package is analyzable.

The specifier is therefore opaque. It contributes no closure edge and no
resolved binding, only the `unaccepted-external-dependency` frontier that an
unaccepted bare dependency contributes, so every claim of this artifact case
stays open: the proposal plan carries twenty open claims and no proof candidate,
where the same entry without the `?raw` import carries three candidates. The
package still produces a proposal with both exports; nothing is refused and
nothing is proven through the loader binding.

`./notify.js` is deliberately still reachable as a module, so the case pins that
the file itself is resolvable and analyzable and that only the query-suffixed
specifier is held opaque. Whether the query-stripped file exists changes nothing:
`./nowhere.js?raw` produces exactly this plan too. A relative specifier with no
query and no file still refuses -- pinned by
`packages/cli/test/artifact-resolution.test.mjs`, "an unsuffixed relative
specifier with no file still refuses the artifact case".
