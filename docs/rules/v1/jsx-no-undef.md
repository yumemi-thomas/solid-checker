# v1/jsx-no-undef

Reports undefined JSX component and `use:` directive names.

Component tags join Oxc JSX spans to TypeScript symbol facts, so imports,
project declarations, and TypeScript globals receive the compiler's answer.
TypeScript does not bind the local-name node of a namespaced JSX attribute;
for `use:name`, the checker instead records the exact value declaration chosen
by Oxc's semantic scope binder. That covers imports, hoisting, parameters,
nested block scope, and shadowing while rejecting type-only declarations.

Dotted component tags remain conservative: one unresolved fact for
`<Object.Component>` cannot distinguish an undefined object from a missing
property, so the rule does not guess which one is wrong.
