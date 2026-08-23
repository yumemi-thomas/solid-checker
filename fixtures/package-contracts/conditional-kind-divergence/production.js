// Under the default branch the same name is an inert value. Export-map order
// decides which branch wins. The contract keeps this as a variant-local kind;
// its conservative base stays a value for environment-unaware consumers.
export const conditionalShape = 42;
