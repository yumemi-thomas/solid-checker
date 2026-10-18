# ADR 0317: An imported base class is resolved

- Status: accepted and implemented (2026-10-17).
- Owners: the heritage demand in `solid-facts-backend/src/demand_plan.rs`;
  its consumer `SemanticLookup::class_instance_is_exact` (and
  `class_chain_prototype_written`, ADR 0314).
- Relation: completes ADR 0209's exact-instance chain for bases declared in
  another module.

## Context

`class_instance_is_exact` follows a class's `extends` expression to its base:
first through the local binder (a base declared in the same file), then
through the compiler symbol at that span. The demand plan never requested a
symbol at an `extends` expression, so an imported base (`class Camera extends
Transform`, `Transform` from `./transform`) had neither: the chain read as
unresolved and every exact-instance proof through it refused. In app-game this
held `ogl`'s `Camera`/`Mesh` subclasses and the paint package's meshes.

## Decision

The demand plan requests the symbol at every class's `extends` expression
(transparent wrappers peeled). No consumer changes: the existing resolution
(`class_for_symbol` of the symbol at the heritage span) now has its fact. The
per-file demand cache keys on the file's path and source hash, which cover
this per-file demand. The plan's completeness test asserts the new demand.

## Consequences

49-project corpus: 5 SC9012 uncertifiable findings removed, all app-game
(`mesh.setColor`, `brushStrokeMesh.setBrushTexture`/`setBrushColor`,
`spotMesh.setColor`, `renderPass.setCamera`), none added. Fixture:
`leaf-scope-imported-base`, where an imported base also lets a forbidden
operation in an exact own method surface as the `SC3001` violation it is.

Still open: inherited methods (`mesh.updateMatrix` declared in `Transform`),
own-field receivers (`this.projectionMatrix.fromPerspective`,
`mesh.position.set`), and classes extending the built-in `Array` (ogl's `Vec3`,
`Mat4`), which together hold most remaining exact-instance leaf roots.
