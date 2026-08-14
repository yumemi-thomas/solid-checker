# v1/valid-jsx-nesting

`SC8020` · **error** · violation

Solid 1.x JSX uses the same browser HTML parser constraints as Solid 2.0.
This rule reports intrinsic nesting that changes the parsed tree and can cause
SSR/hydration mismatches. It stops at component boundaries rather than guessing
what DOM a component returns.

See the shared [valid-jsx-nesting](../valid-jsx-nesting.md) documentation for
examples and the exact scope.
