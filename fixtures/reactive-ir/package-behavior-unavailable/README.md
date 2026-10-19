# Package behavior unavailable (SC9005)

Pins the one live package-consumer answer: an import from a package that declares a `solid-js` peer dependency, with declarations but no analyzable source and no declared runtime host, gets a single `SC9005` uncertifiable result at the import. The call is never reported as a violation. This is the only fixture here that exercises SC9005 on a consumer; the other package-consumer fixtures were deleted when package contracts were removed (ADR 0312) because they no longer distinguished anything.
