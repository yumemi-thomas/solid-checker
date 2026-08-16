# Package-contract torture corpus

These five packages are deliberately small release blockers for contract
generation. Their `expected.json` files are normalized output pins; the corpus
runner never updates them automatically.

| Fixture | Shape under test | Safety assertion |
| --- | --- | --- |
| `torture-runtime-namespace` | runtime-mutated object namespace and getter | the mutable namespace remains an unstructured value |
| `torture-conditional-semantics` | browser and node exports with different callback timing | both conditional callback behaviors are retained |
| `torture-getter-exports` | getter-backed object/function exports | getter syntax does not fabricate a reactive claim; the real memo return stays explicit |
| `torture-deep-barrel` | two-level `export *` and named re-export barrel | the exact deep declaration is followed once |
| `torture-dts-disagreement` | declarations disagree with the runtime module | runtime exports win; declaration-only names never become claims |

Run `make contract-corpus` to regenerate each package in a temporary
directory, compare its output with the checked-in pin, and collect V8 execution
ranges for the generator. A mismatch is a required review of the expected
contract, not an invitation to broaden a claim.
