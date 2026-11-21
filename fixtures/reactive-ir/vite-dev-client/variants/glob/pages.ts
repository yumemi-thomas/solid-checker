// Review F2: `import.meta.glob` loads modules no specifier names.
export const pages = import.meta.glob("./*.tsx");
