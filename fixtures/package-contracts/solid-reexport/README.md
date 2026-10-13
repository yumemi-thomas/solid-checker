# A renderer that re-exports `solid-js` publishes nothing of its own

`custom-solid-renderer` publishes `createMemo` and `createSignal` straight from
its `solid-js` dependency. The dialect vocabulary knows both names, and the
bundled Solid contracts describe both -- so this is exactly the package where
the temptation is to bind the re-export by name and publish the known
semantics under this package's identity.

It publishes neither, and the refusal says why: the entry file has no runtime
ESM exports *of its own*. Both names are dropped from the surface before
attribution, because `solid-js`, `@solidjs/signals` and `@solidjs/web` have no
package contract **by design** (ADR 0027) -- ordinary analysis takes their
behavior from the selected dialect, and `core_runtime_contract_reference`
withholds a contract for them. A name this package only re-exports from that
foundation is the dialect's to describe; this package has no standing to
describe it, and nothing could ever bind it.

Name-based trust is still refused, which is the point this fixture has always
pinned: nothing here republishes Solid's semantics under
`custom-solid-renderer`'s identity. A consumer calling
`custom-solid-renderer`'s `createMemo` gets no contract claim from this package
at all, which is ADR 0027's "missing native behavior stays unknown".

## Why the refusal changed on 2026-09-15

This fixture predates ADR 0027 by four days. It used to pin

```
accepted dependency solid-js has no exact runtime binding for export createMemo
```

on the premise that an accepted contract for `solid-js` would bind the
re-export. ADR 0027 made that premise unreachable: core has no contract that
can be supplied, so the demand could never be met by anything. Worse, the
demand refused the **whole artifact case**, so a package with one core
re-export beside a hundred of its own published nothing -- which is what left
`@solid-primitives/utils@6.4.1`'s `.` entrypoint, and the 820 consumer call
sites that import it, with no contract at all.

Three censuses now agree that a core-only re-export is not part of a package's
surface, and they must keep agreeing:

- the emitter drops the name (`export_binds_core_runtime` in `main.rs`);
- the resolver returns it unbound (`bindExport`'s `coreRuntimeSpecifier` arm in
  `artifact-resolution.mjs`), which the caller already skips;
- `bind_exports` in `artifact_resolution.rs` stays **strict**, so a document
  that still names a core export -- stale, or hand-edited -- refuses rather
  than being silently accepted.

`external-reexport` pins the unchanged half: an ordinary dependency is not
exempt, and its re-export still refuses by name, because that contract *can* be
supplied and was not. The `node_modules/solid-js` stub is 2.0.0-rc.3; it is the
fixture whose missing `.gitignore` exception motivated coverage's
`checkDialectStubs` guard.

The refusal is dialect-independent, and this fixture is the proof: ported from
the 1.x stub to 2.0.0-rc.3, the refusal text and the census sidecar came back
**byte-identical**, with no snapshot to update. ADR 0027 withholds a contract
for `solid-js`, `@solidjs/signals` and `@solidjs/web` by design, and "by design"
does not mean "per dialect".
