# Package parameter handed to an undescribed callee (ADR 0370)

Each `dep-run` export passes its parameter to a function value no source defines (direct, through an alias, destructured, helper-returned, second module, closure, re-export, object method): SC9011 at each `count`. `run(() => 1)` passes no source.
