# async-options-unreadable

ADR 0388 (first half): `async-outside-loading-boundary` (`SC5003`) reports a bare render of an async source as a violation only when the source's options are readable. An options argument that is a variable or carries a spread may declare `loadingValue`/`seedLoadingValue`, so the bare render is `uncertifiable`, as `SC5001` already was.

- `OpaquePage`: `variableUser` (options variable) and `spreadUser` (`{ ...opaqueOptions }`) → SC5003 uncertifiable.
- `LiteralPage`: a literal without the key and a memo with no options → SC5003 violations (every chain reaches a `render` root with no boundary).
- `DeclaredPage`: a literal `loadingValue` → no finding.

Stub as `fp-owner-loading-cover`.
