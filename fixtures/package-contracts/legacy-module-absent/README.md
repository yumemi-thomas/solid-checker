# A declared `module` target that the artifact does not contain

`module` names `dist/esm/index.js`, which was never published. `main` names a
real ESM file.

A missing `module` target is not a refusal. Node consumers never read `module`
at all, so the `main` surface is the one every runtime consumer actually loads
and it is still a real, analyzable artifact. Legacy runtime resolution
therefore falls back to `main` and records `legacy:main`, exactly as it did
before `module` was consulted.

The same fallback covers a `module` value that is not a usable target at all --
a traversal, an escaping path, or a non-string -- because none of those name a
file inside the artifact. `legacy-module-entry` pins the case where the target
is present and wins.
