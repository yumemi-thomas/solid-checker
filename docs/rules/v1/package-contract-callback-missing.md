# v1/package-contract-callback-missing

`SC9006` · **error** · uncertifiable

An exported callback reaches an external helper whose runtime execution timing
is not described by a package contract. Solid 1.x timing is not inferred from
the helper's name or its TypeScript signature.

Use the package, entrypoint, function, parameter type, and JSON stub shown in
the finding to author an exact contract. If the helper's source is available,
start with:

```sh
solid-checker contract generate --package-root <package-root> --entrypoint <entrypoint>
```

The checker stays uncertifiable until the reviewed contract explicitly records
`inline`, `tracked`, or `deferred` execution.
