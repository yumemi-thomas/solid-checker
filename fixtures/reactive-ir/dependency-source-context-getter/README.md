# A package's context-provided getter (ADR 0289)

Claim: with `--dependency-source dep-router` (`.solid-checker/runtime.json`
`dependencySources`), a project Get of a member that the package defines as a
getter reading an accessor is an **uncertifiable** SC9012 obligation, never
a violation, when it runs in a role that reports an untracked read. Before
ADR 0289 it was silent.

`dep-router` is the shape of `@solidjs/router` 2.0.0-next.26, which kui
reads (`TopicsRoute.tsx:425`): `useLocation = () => useRouter().location`,
`useRouter = () => invariant(useContext(RouterContext), …)`, the package's
own `<Router>` provides `createRouterState()`, whose `location` member is
`createLocation(path)`, an object literal with `get search() { return
search(); }` over a memo. Like the router, every runtime file has a `.d.ts`
beside it, so the compiler binds `./routing.js` to `routing.d.ts` and ADR
0284 redirects it; the context census has to follow that redirect to see
`<RouterContext value={state}>` in `router.jsx` at all.

- `SearchNow`, `SearchThroughRouter`, `SearchDestructured`, `SearchComputed`
  (obligations): the Get runs `search` in the component body, through a
  local, an inline chain, a destructuring pattern and a literal computed key.
- `SearchTracked`, `SearchMemo`, `SearchOnClick` (clean): JSX, a memo and an
  event handler are no untracked-read role.
- `Inert` (clean): `pending` and `key` are getters that read nothing
  reactive, and `base` is a data member.
- `SearchShadowed` (clean): a project hook returning a plain object is not
  the package's; exact identity decides, not the name.
- `ProjectGetter` (scope control): a getter object the project itself
  creates is outside this change. Its Get stays silent, as before; the gap is
  recorded in docs/precision-backlog.md.

None of these is a violation. The checker carries no getter through a
package's source: it projects only callable members, one level deep, of
fully data-only literals, and `utils.js`'s `Object.assign(target, source)` —
the router has `Object.assign(obj, written)` — is ADR 0284's wildcard veto on
every member of the objects the package creates. Without the flag the
package has no contract and its declarations type every member as `string`,
so the same sources report nothing.

`solid-js.d.ts` is `context-member-identity`'s stub (claim-bearing
signatures copied from published 2.0.0-rc.13). `App.tsx` passes
`tsc --noEmit`; the package's JavaScript is not type-checked
(`checkJs: false`).
