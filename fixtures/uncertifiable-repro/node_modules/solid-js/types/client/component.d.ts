import type { Element as SolidElement } from "../types.js";
/**
 * A general `Component` has no implicit `children` prop. If desired, specify
 * one explicitly, e.g. `Component<{ name: string; children: Element }>`.
 */
export type Component<P extends Record<string, any> = {}> = (props: P) => SolidElement;
/**
 * Extend props to forbid the `children` prop.
 * Use this to prevent accidentally passing `children` to components that
 * would silently throw them away.
 */
export type VoidProps<P extends Record<string, any> = {}> = P & {
    children?: never;
};
/**
 * `VoidComponent` forbids the `children` prop.
 * Use this to prevent accidentally passing `children` to components that
 * would silently throw them away.
 */
export type VoidComponent<P extends Record<string, any> = {}> = Component<VoidProps<P>>;
/**
 * Extend props to allow optional Solid children.
 * Use this for components that you want to accept children.
 */
export type ParentProps<P extends Record<string, any> = {}> = P & {
    children?: SolidElement;
};
/**
 * `ParentComponent` allows an optional `children` prop.
 * Use this for components that you want to accept children.
 */
export type ParentComponent<P extends Record<string, any> = {}> = Component<ParentProps<P>>;
/**
 * Extend props to require a `children` prop with the specified type.
 * Use this for components where you need a specific child type,
 * typically a function that receives specific argument types.
 */
export type FlowProps<P extends Record<string, any> = {}, C = SolidElement> = P & {
    children: C;
};
/**
 * `FlowComponent` requires a `children` prop with the specified type.
 * Use this for components where you need a specific child type,
 * typically a function that receives specific argument types.
 */
export type FlowComponent<P extends Record<string, any> = {}, C = SolidElement> = Component<FlowProps<P, C>>;
export type ValidComponent = Component<any>;
/**
 * Takes the props of the passed component and returns its type
 *
 * Intrinsic element prop extraction is renderer-specific and lives in renderer
 * packages such as `@solidjs/web`.
 */
export type ComponentProps<T extends ValidComponent> = T extends Component<infer P> ? P : never;
/**
 * Type of `props.ref`, for use in `Component` or `props` typing.
 *
 * @example Component<{ref: Ref<Element>}>
 */
export type Ref<T> = T | ((val: T) => void) | undefined | Ref<T>[];
/**
 * Invokes a component, wrapping the call in `untrack` so that reactive reads
 * inside the component body don't subscribe the parent computation. Compiled
 * JSX uses this internally; manual calls are rarely needed unless authoring a
 * custom JSX factory or renderer.
 *
 * @param name the tag as written in source (`"Home"`, `"Ui.Button"`), emitted
 *   by the compiler's `sourceNames.components` option. Dev and observe builds label
 *   the component's owner with it — `Comp.name` is whatever the minifier left
 *   — so diagnostics and attribution paths read `<Home>` in production
 *   bundles. The production build ignores it.
 * @param loc the JSX site as written in source (`"src/App.tsx:12:5"`,
 *   root-relative `file:line:col`), emitted by the compiler's `sourceNames`
 *   `locations` kind beside `name`. Dev and observe builds keep it on the
 *   component's owner, so a diagnostic raised inside the component reports
 *   `at src/App.tsx:12:5` when nothing closer was located. The production
 *   build ignores it.
 */
export declare function createComponent<T extends Record<string, any>>(Comp: Component<T>, props: T, name?: string, loc?: string): SolidElement;
/**
 * Defines a code-split component. The returned component triggers its dynamic
 * import on first render and suspends through any enclosing `<Loading>`
 * boundary while the chunk is in flight. Call `.preload()` to start the
 * import early (e.g. on hover).
 *
 * @param fn dynamic import resolving the component's module. By default the
 *   component must BE the module's default export (same contract as
 *   React.lazy): hydration resolves it synchronously from the preloaded
 *   module, so wrappers that select a named export at runtime are not
 *   supported. To use a named export, pass `{ export: "Name" }` — the name is
 *   a call-site literal available on both server and client, so the
 *   synchronous hydration claim is preserved.
 * @param options `{ export?: string }` — which export of the resolved module
 *   is the component (defaults to `"default"`).
 * @param moduleUrl optional module specifier injected by the bundler
 *   integration; exposed as the component's `moduleUrl` property (islands)
 *   and used in hydration error messages. Hydration itself matches
 *   server-preloaded modules positionally by hydration id, so lazy
 *   components without a moduleUrl (e.g. via import.meta.glob) hydrate too.
 *
 * @example
 * ```tsx
 * const Profile = lazy(() => import("./Profile"));
 * const About = lazy(() => import("./pages"), { export: "About" });
 *
 * function App() {
 *   return (
 *     <Loading fallback={<Spinner />}>
 *       <Profile id="42" />
 *     </Loading>
 *   );
 * }
 *
 * // Preload before the user clicks
 * <button onMouseEnter={() => Profile.preload()}>Open profile</button>
 * ```
 */
export declare function lazy<M extends Record<string, any>, K extends keyof M & string>(fn: () => Promise<M>, options: {
    export: K;
}, moduleUrl?: string): M[K] & {
    preload: () => Promise<M>;
    moduleUrl?: string;
};
export declare function lazy<T extends Component<any>>(fn: () => Promise<{
    default: T;
}>, options?: {
    export?: string;
}, moduleUrl?: string): T & {
    preload: () => Promise<{
        default: T;
    }>;
    moduleUrl?: string;
};
/**
 * A cached dynamic import the server can preload before hydration, for a
 * module that is not itself a component. `lazy()` is the component-shaped
 * sibling and is unchanged.
 *
 * Call the result (or `.preload()`) to start the import. Both share one
 * promise. `.peek()` reads it synchronously: the settled module, or
 * `undefined` when it has not landed. During hydration `.peek()` reads
 * `_$HY.modules[moduleUrl]` and returns `undefined` on a miss — it does not
 * throw. The bundler pass appends `moduleUrl` for a literal `import()`.
 *
 * @example
 * ```ts
 * const admin = lazyModule(() => import("./admin/routes"));
 * admin();          // Promise<Module>, cached
 * admin.preload();  // same promise
 * admin.peek();     // Module | undefined
 * ```
 */
export declare function lazyModule<T extends Record<string, any>>(fn: () => Promise<T>, moduleUrl?: string): (() => Promise<T>) & {
    preload: () => Promise<T>;
    peek: () => T | undefined;
    moduleUrl?: string;
};
/**
 * Returns a stable id string that matches between server-rendered and
 * client-hydrated trees. Use it for `<label for>`, `aria-labelledby`, and
 * other attributes that need consistent ids across SSR.
 *
 * @example
 * ```tsx
 * function Field(props: { label: string }) {
 *   const id = createUniqueId();
 *   return (
 *     <>
 *       <label for={id}>{props.label}</label>
 *       <input id={id} />
 *     </>
 *   );
 * }
 * ```
 */
export declare function createUniqueId(): string;
