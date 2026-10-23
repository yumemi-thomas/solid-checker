import type { Component, ComponentProps } from "./component.js";
/**
 * Options for `dynamicComponent`. Same members as the client type; `solid-js`
 * publishes the client declaration.
 */
export interface DynamicOptions {
    /**
     * SSR only: hold the document's first flush until the source settles, so
     * the resolved component renders into the shell instead of streaming in
     * behind its boundary's fallback. Same meaning as `createMemo`'s
     * `deferStream`. Default `false`.
     */
    deferStream?: boolean;
    /**
     * The source cannot change: call it once, untracked, now, and render the
     * result with no computation per instance. The source must resolve
     * synchronously.
     */
    static?: boolean;
}
/**
 * The server twin of `dynamicComponent`. Same owner shape as `dynamic` on
 * both sides (factory / value / render memos), so hydration ids agree
 * whichever of the two mounts the same value. One signature with the client
 * (including `AsyncIterable`); `solid-js` ships a single types file.
 */
export declare const dynamicComponent: {
    <C extends Component<any>>(source: () => C | Promise<C> | AsyncIterable<C> | null | undefined | false, options?: DynamicOptions): Component<ComponentProps<C>>;
};
