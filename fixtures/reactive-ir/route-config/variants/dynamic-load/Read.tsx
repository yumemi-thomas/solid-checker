// A load whose specifier is not a literal can load the router itself.
const name: string = "@solidjs/router";
export const load = () => import(name);
