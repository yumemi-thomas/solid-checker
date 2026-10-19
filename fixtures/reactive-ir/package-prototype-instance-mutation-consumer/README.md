# Cross-file prototype mutation

`Mutate.ts` replaces the imported constructor's `has` method before `App` uses a fresh instance, so the exact instance proof
is withheld as an SC9012 uncertifiable obligation. This is an isolated authorized fixture so its class mutation does not
taint the positive project.
