/**
 * Resolves nested accessors and arrays in a children tree to their rendered
 * values. See the `children()` helper in solid-js for the memoized form.
 */
export declare function flatten(children: any, options?: {
    skipNonRendered?: boolean;
    doNotUnwrap?: boolean;
}): any;
