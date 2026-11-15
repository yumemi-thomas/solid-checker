# after-await-handler-forms

**Claim (ADR 1003).** A promise handler written another way than a function literal or a function's name (ADR 0976, S3) is followed to the functions it may be, and
the reads in each are after a suspension. Where the walk cannot name every function, the handler argument itself is an uncertifiable SC1002 row: what runs after
the suspension is not known.

| function | outcome |
|---|---|
| `viaMember` (`then(handlers.ok)`) | SC1002 uncertifiable at the read in `handlers.ok`, and at the argument (nothing proves the object kept the function) |
| `viaConditional` (`then(c ? other : reader)`) | SC1002 uncertifiable at the read in `other` and in `reader` |
| `viaParameter` (`chain(() => source())`, `then(handler)` in `chain`) | SC1002 uncertifiable at the read in the function the caller hands, and at the argument in `chain` (a caller the program does not name may hand any function) |
| `viaBound` (`then(bound.bind(null))`) | SC1002 uncertifiable at the read in `bound` |
| `viaSpread` (`then(...fns)`) | SC1002 uncertifiable at the read in the element of the array |
| `viaResult` (`then(make())`) | SC1002 uncertifiable at the argument |
| `viaExecutor` (`new Promise((resolve, reject) => { pause().then(resolve, reject); })`) | no row: the library's own `resolve` and `reject` |
| `viaNullish` (`then(undefined, null)`) | no row |

Chrome: `fixtures/runtime-oracle/after-await-handler-forms` (the first five; the last three raise nothing).
