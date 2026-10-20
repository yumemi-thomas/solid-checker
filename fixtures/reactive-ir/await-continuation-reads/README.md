# Reads after an unconditional await (ADR 0323)

The synchronous prefix of an async callback runs in the render window; what follows an await reached on every path runs on a later task where `strictRead` is restored (`@solidjs/signals@2.0.0-rc.13` `dist/dev-shared.js:5801-5885`), so its role is no-claim: before-await reads are reported, after-await reads silent, and skippable awaits (`x ??= await p`, `f?.(await p)`, labelled `break`) still report. Published rc.13 declarations from `standard-runtime-configuration`.
