# Project parameter handed to an undescribed callee (ADR 0369)

A project helper (local, or two hops in another module) passes its parameter to a package function with no analyzed body or to an undefined function value: SC9011 at each `count`. `forward(count)` reaches only `createMemo`: clean.
