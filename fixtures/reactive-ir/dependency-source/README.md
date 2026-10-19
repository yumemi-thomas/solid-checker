# Dependency source (ADR 0284)

With `--dependency-source dep-hooks` the package's runtime source is analyzed as project code instead of requiring a contract: `LabelNow` binds to the file `exports` selects under the `solid` condition and reaches `readNow` (SC1001 at the call); `LabelMemo` defers into a memo; `Badged`'s own read is located in the package (not reported); `Greeting` still transports (the package's open `Object.assign` vetoes only objects the package creates, ADR 0273). `runtime.json` declares a browser development host; without the flag the sources report SC9011.
