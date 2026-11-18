# jsx-helper-child-component

**Claim (ADR 1091).** A helper `const` in a component's body, read only in a JSX child, is placed by the render when a tag in the same file renders the component. The hidden-hand-off check (`function_escapes_unfollowed`, ADRs 0673/0674) read the component's same-file tag as a hand-off nothing follows, so `shown()` in `Content` was uncertifiable (`function-run-unplaced`), while the same component mounted from another file was proved. A reference that is a JSX element's tag is now no hand-off for that check; every other reference keeps its class.

| component | outcome |
|---|---|
| `Content` | proved: no row |
| `Copied` (also copied into a `const` and called) | SC1001 uncertifiable, `function-run-unplaced` |
| `Listed` (also in an exported array) | SC1001 uncertifiable, `function-run-unplaced` |
| `BodyRead` (the helper read in the body) | SC1001 violation |

The fuzzer's legal grid holds the shape as the `jsx-helper` context of STRICT_READ_UNTRACKED (mounts `root`, `child`, `show`, `for`). `solid-js.d.ts` is `async-creation-pass`'s.
