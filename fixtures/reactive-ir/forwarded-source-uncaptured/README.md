# Project parameter handed to an undescribed callee (ADR 0370)

Project helpers (one two hops away) pass their parameter to a package function with no body or an undefined function value: SC9012 at each `count`. `forward(count)` reaches only `createMemo`: clean.
