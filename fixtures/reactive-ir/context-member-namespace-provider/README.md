# Context member through a namespace-member provider

Negative for `context-member-identity`, in a closed program. `FixedTheme`
renders a second `ThemeContext` provider through the namespace member
`themes.ThemeContext`. That tag is not one exact identifier, so it cannot be
attributed to one context binding. No provider census in the project is
closed, and `ThemeLabel`'s read is not attributed to the signal.

The veto is project-wide: an unkeyed provider withholds every context, so the
single-provider positive lives in its own project (`context-member-identity`,
whose `TabsList` read is the same shape).

`solid-js.d.ts` is the `context-member-identity` stub. The sources pass
`tsc --noEmit` (strict, skipLibCheck false) against installed rc.13.
