# Own-setter write through a TypeScript-wrapped setter

Claim: a wrapped callee (`setStore!`, `setStore as typeof setStore`,
`satisfies`, `<typeof setStore>setStore` in a .ts file, and any nesting of
them) is the setter, because transparent TypeScript wrappers preserve the
value. Reference lookup peels them to the exact identifier, so a write through the store's
own proxy inside its own setter's draft callback stays silent. A write inside
another store's (wrapped) setter is still SC2003.

Only a callee proven not to be the own setter keeps the violation: another
primitive-created tuple's setter, or a standard-library function (`forEach`;
`call`/`apply`/`bind`/`Reflect.apply` excluded, since they invoke a function
value they are handed). A callee that may still be the own setter at runtime
(an alias, an object member, `setStore.bind(null)`, or a project function
that may forward its callback) makes the write uncertifiable, not a dropped
write.

The project is an application (`"private": true`) on rc.13. `solid-js.d.ts`
adds the plain `createStore` form copied from published
`@solidjs/signals@2.0.0-rc.13`. The sources pass `tsc --noEmit` (strict,
skipLibCheck false) against installed rc.13; a browser/development runtime
probe of the asserted form commits the write (review artifact,
rust/target/research/context-member/REVIEW-1.md).
