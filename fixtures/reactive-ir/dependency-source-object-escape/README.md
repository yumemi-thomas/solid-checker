# ADR 0299 review: namespace

The runtime object escapes through a namespace of an `export *` barrel into
opaque code. Neither reader.readCount nor timer.useTicker may supply a proven
read or cleanup at App. Both are violations on ba3d9085d and silent after the
shared receiver census refuses the export channel. This also joins the .d.ts
receiver's references to its redirected runtime symbol.

The review's direct namespace import of dep-object is already refused by
ADR 0297 selection on ba3d9085d. The barrel exposes the same hole while keeping
dependency source selected, without modifying that separate selection owner.
Solid signatures are copied verbatim from object-member-import's published
rc.13 declarations. Both tiny packages carry their full runtime and faithful
declarations. All application sources pass tsc against the real rc.13 typings.
