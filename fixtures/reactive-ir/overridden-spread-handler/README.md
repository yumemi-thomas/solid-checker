# overridden-spread-handler

ADR 1140. Where an intrinsic element has a spread, the compiler hands its spreads and attributes to `spread` as one list of sources in
source order, and rc.21 `@solidjs/web` `collectProps` skips a key that a later source has (`sourceHas`, whatever the value, `undefined`
too). The function an earlier source holds under that key is never attached, so its body never runs from the element.

`Overridden`: `createRenderEffect` in a literal-spread `onClick` that a later attribute, a later spread or a later `onClick={undefined}`
overrides is no longer an SC4001 `missing-owner` violation but an obligation, `handler-key-overridden`. On main a64fc92ad each was a
violation, and Chrome never reports one there (runtime-oracle `overridden-spread-handler`). `AttributeOverridden`: an attribute handler
that a later spread may supply under its key (a binding typed `{ onClick?: … }`) moves the same way. An attribute written before a
literal spread holding its key is TS2783, so that shape is not used.

`StillAttached` keeps every violation: no later source has the key (a later `title`, a later literal spread without it, a later
`onDblClick`), or only an earlier source has it.
