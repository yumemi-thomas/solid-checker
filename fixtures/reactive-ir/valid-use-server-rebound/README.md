# SC7006: registration is the initializer at the declaration

ADR 0702. `@solidjs/compiler@2.0.0-rc.21` `transformDirectives` in server mode rewrites each export of a module-level
`"use server"` file as `registerServerReference(id, <initializer>)` at the declaration, the specifier form (`export { g }`)
included, so a later reassignment never reaches registration. Today's answers are right and are pinned here: a rebound function
export is clean (`rebound-function.ts`); a literal export rebound to a function is still a violation (`rebound-literal.ts`); a
member write changes nothing, and a literal export is a violation (`kept.ts`).
