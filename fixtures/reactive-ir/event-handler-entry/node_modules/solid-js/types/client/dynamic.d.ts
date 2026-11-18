import type { Component, ComponentProps } from "./component.js";
/**
 * Options for `dynamicComponent` (and for `@solidjs/web`'s `dynamic`, which
 * passes them through to the same core).
 */
export interface DynamicOptions {
    /**
     * SSR only: hold the document's first flush until the source settles, so
     * the resolved component renders into the shell instead of streaming in
     * behind its boundary's fallback. Same meaning as `createMemo`'s
     * `deferStream`. Default `false` — a source is data of unknown cost and
     * streams by default. Ignored on the client.
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
 * `dynamic` for a source that only ever answers with a component — never a
 * tag name. A reactive, optionally async source; a stable `Component` back;
 * `{ static: true }` and `{ deferStream: true }`. The source's type excludes
 * strings, so a tag name is a compile error here — use `@solidjs/web`'s
 * `dynamic` for one.
 *
 * Cost model, which is the reason this exists: `dynamic` must be able to
 * render a tag, so one `dynamic` anywhere on a page retains the element
 * runtime — `createElement`, `spread` and the prop-collection helpers, the
 * SVG/MathML tables — for everyone, whether or not any source ever answers
 * with a string (a bundler cannot know what a source will resolve to).
 * `dynamicComponent` has no tag arm and never references that runtime, so a
 * page whose only dynamic mounts are components pays nothing for it.
 *
 * This is the documented way to mount a server component: a server
 * function's answer is a component reference, and the frames transport
 * resolves every call for the same function to the same mount identity, so
 * a refetch or an argument change is delivered into the mounted instance
 * rather than remounting it.
 *
 * @example
 * ```tsx
 * const Story = dynamicComponent(() => getStory(props.storyId));
 * return <Story comment={p => <Comment cid={p.cid}>{p.children}</Comment>} />;
 *
 * // A client component chosen by (sync) data — the same thing `dynamic`
 * // does, without retaining the element runtime for the page.
 * const Page = dynamicComponent(() => (page().editable ? Editor : Viewer));
 * ```
 */
export declare const dynamicComponent: {
    <C extends Component<any>>(source: () => C | Promise<C> | AsyncIterable<C> | null | undefined | false, options?: DynamicOptions): Component<ComponentProps<C>>;
};
