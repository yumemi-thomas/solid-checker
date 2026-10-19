# release-triple-solid-dependent-rc3

A project with no Solid code importing `solid-format`, whose manifest peer-depends on `solid-js`, on an unaudited `2.0.0-rc.3` triple: the census reads each reached package's installed dependency closure and a closure naming a Solid 2 package keeps the notice (`release_scope.rs`, `solid_use`). Expected: `SC9014` (the unknown `format` call also raises `SC9005`).
