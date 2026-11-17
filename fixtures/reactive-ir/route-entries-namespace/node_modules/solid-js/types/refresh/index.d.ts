import type { Element as SolidElement } from "../types.js";
export interface BaseComponent<P> {
    (props: P): SolidElement;
}
/** Mutable live-mount counter shared between a proxy and its registration. */
interface LiveInstances {
    count: number;
}
interface ComponentOptions {
    location?: string;
    signature?: string;
    dependencies?: () => Record<string, any>;
}
export interface ComponentRegistrationData<P> extends ComponentOptions {
    id: string;
    component: (props: P) => SolidElement;
    proxy: (props: P) => SolidElement;
    update: (action: () => (props: P) => SolidElement) => void;
    /** Live renders through this registration's proxy (owner not yet disposed). */
    instances: LiveInstances;
}
export interface Registry {
    components: Map<string, ComponentRegistrationData<any>>;
}
export declare function $$registry(): Registry;
export declare function $$component<P extends Record<string, any>>(registry: Registry, id: string, component: (props: P) => SolidElement, options?: ComponentOptions): (props: P) => SolidElement;
declare const SOLID_REFRESH = "solid-refresh";
declare const SOLID_REFRESH_PREV = "solid-refresh-prev";
type HotData = {
    [key in typeof SOLID_REFRESH | typeof SOLID_REFRESH_PREV]: Registry;
};
/**
 * Which hot API the compiled wrapper hands the runtime: `"vite"` — the
 * `import.meta.hot` shape (`ESMHot`); `"standard"` — the `module.hot` /
 * `import.meta.webpackHot` shape (`StandardHot`).
 */
export type RuntimeType = "vite" | "standard";
interface ESMHot {
    data: HotData;
    accept: (cb: (module?: unknown) => void) => void;
    invalidate: () => void;
}
interface StandardHot {
    data: HotData;
    accept: (cb?: () => void) => void;
    dispose: (cb: (data: HotData) => void) => void;
    invalidate?: () => void;
    decline?: () => void;
}
export interface RefreshConfig {
    /**
     * Called when the runtime bails out of hot-swapping (the module must be
     * fully re-executed, e.g. an exported component vanished and the bundler
     * lacks an invalidate/decline API). Overrides the default bail behavior:
     * `hot.invalidate()` when available → `window.location.reload()` in DOM
     * environments → one-time console warning otherwise.
     */
    invalidate?: (hot?: {
        invalidate?: () => void;
    }) => void;
}
/**
 * Configures runtime behavior that is not part of the frozen compiled-wrapper
 * ABI. Intended to be called once by bundler integrations (@solidjs/vite-plugin)
 * before any hot update fires.
 */
export declare function configureRefresh(config: RefreshConfig): void;
type ESMDecline = [type: "vite", hot: ESMHot, inline?: boolean];
type StandardDecline = [type: "standard", hot: StandardHot, inline?: boolean];
type Decline = ESMDecline | StandardDecline;
export declare function $$decline(...[type, hot, inline]: Decline): void;
type ESMRefresh = [type: "vite", hot: ESMHot, registry: Registry];
type StandardRefresh = [type: "standard", hot: StandardHot, registry: Registry];
type Refresh = ESMRefresh | StandardRefresh;
export declare function $$refresh(...[type, hot, registry]: Refresh): void;
export {};
