import type { Accessor, EffectOptions } from "./signals.js";
import type { ArrayElement, Element as SolidElement } from "../types.js";
import type { FlowComponent } from "./component.js";
export declare const $DEVCOMP: unique symbol;
export type NoInfer<T extends any> = [T][T extends any ? 0 : never];
export type ContextProviderComponent<T> = FlowComponent<{
    value: T;
}>;
export interface Context<T> extends ContextProviderComponent<T> {
    id: symbol;
    defaultValue: T | undefined;
}
/**
 * Creates a Context to share state with descendants of a Provider.
 *
 * - `createContext<T>()` — default-less. `useContext` throws
 *   `ContextNotFoundError` outside any Provider. Use this for any context
 *   carrying reactive state.
 * - `createContext<T>(defaultValue)` — default form. `useContext` falls back
 *   to `defaultValue` outside a Provider. Reserved for primitive fallbacks
 *   (theme, locale, frozen config).
 *
 * @param defaultValue optional default; only meaningful for primitive fallbacks
 * @param options allows to set a name in dev mode for debugging purposes
 */
export declare function createContext<T>(defaultValue?: T, options?: EffectOptions): Context<T>;
/**
 * Reads the current value of a context. Throws `ContextNotFoundError` if no
 * Provider is mounted and the context was created without a default.
 *
 * Inside a server component the miss is upgraded to an error explaining the
 * boundary: app-level providers exist at t=0 (the component renders inline
 * in the document) but not on refetches or mutation regions (it renders
 * standalone), so context deliberately never crosses a server-component
 * root — surfacing "no provider" there would point at the wrong problem.
 */
export declare function useContext<T>(context: Context<T>): T;
export type ResolvedElement = Exclude<SolidElement, ArrayElement>;
export type ResolvedChildren = ResolvedElement | ResolvedElement[];
export type ChildrenReturn = Accessor<ResolvedChildren> & {
    toArray: () => ResolvedElement[];
};
/**
 * Resolves child elements to help interact with children
 * @param fn an accessor for the children
 * @returns a accessor of the same children, but resolved
 */
export declare function children(fn: Accessor<SolidElement>): ChildrenReturn;
