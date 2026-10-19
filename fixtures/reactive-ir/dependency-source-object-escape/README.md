# ADR 0299 review: namespace

The runtime object escapes through a namespace of an `export *` barrel into opaque code, so neither `reader.readCount` nor `timer.useTicker` may supply a proven read or cleanup at `App` (the shared receiver census refuses the export channel and joins the `.d.ts` receiver's references to its redirected runtime symbol). Solid signatures are copied verbatim from `object-member-import`'s published rc.13 declarations; both tiny packages carry full runtime and faithful declarations.
