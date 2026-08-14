# package-contract-callback-missing

`SC9006` · **error** · uncertifiable

An exported callback reaches an external helper whose runtime execution timing
is not described by a package contract. The checker refuses to assume inline,
tracked, or deferred execution.

The finding includes the exact package entrypoint, function, callback parameter
type, required execution choice, and an editable JSON stub. Audit the helper,
replace the stub placeholders, and install the reviewed contract at the
package's `solid-reactivity.json` location. Until that contract is explicit,
the result remains uncertifiable.

When package source is available, generate a starting contract with:

```sh
solid-checker contract generate --package-root <package-root> --entrypoint <entrypoint>
```
