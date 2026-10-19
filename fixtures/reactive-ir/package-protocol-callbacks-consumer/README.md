# A consumer of non-call `callbacks` items

An accepted contract whose closed `callbacks` includes non-call items (`protocol: get`/`coerce`) leaves nothing open (open `callbacks` raise `SC9005`), and no pass reads a non-call item as an invocation of the argument (`ContractCallback::is_invocation`): `access` is uncertifiable in a body and clean in JSX/handlers; `compare` (`coerce`) clean; spread slots and shadows no read; `accessAlways` (fixture-only `min: 1`) a body violation. `index.d.ts` declares `access`/`compare` as `@solid-primitives/utils@7.0.0-next.4` does.
