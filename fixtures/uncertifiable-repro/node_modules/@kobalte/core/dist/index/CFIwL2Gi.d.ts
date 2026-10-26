import { ComponentProps, JSX, ValidComponent } from "@solidjs/web";
//#region src/polymorphic/polymorphic.d.ts
/**
 * Allows for extending a set of props (`Source`) by an overriding set of props (`Override`),
 * ensuring that any duplicates are overridden by the overriding set of props.
 */
type OverrideProps<Source = {}, Override = {}> = Omit<Source, keyof Override> & Override;
/**
 * Allows for extending a set of `ComponentProps` by an overriding set of props,
 * ensuring that any duplicates are overridden by the overriding set of props.
 */
type OverrideComponentProps<T extends ValidComponent, P> = OverrideProps<ComponentProps<T>, P>;
type ElementOf<T> = T extends HTMLElement ? T : T extends keyof HTMLElementTagNameMap ? HTMLElementTagNameMap[T] : any;
/**
 * Polymorphic attribute.
 */
interface PolymorphicAttributes<T extends ValidComponent> {
  as?: T | keyof JSX.HTMLElementTags;
}
/**
 * Props used by a polymorphic component.
 */
type PolymorphicProps<T extends ValidComponent, Props extends {} = {}> = OverrideProps<ComponentProps<T> // Override props from custom/tag component with our own
,
// Override props from custom/tag component with our own
Props // Accept custom props of our own component
 & PolymorphicAttributes<T>>;
/**
 * Helper type to get the exact props in Polymnorphic `as` callback.
 */
type PolymorphicCallbackProps<CustomProps extends {}, Options extends {}, RenderProps extends {}> = Omit<CustomProps, keyof Options | keyof RenderProps> & RenderProps;
/**
 * A utility component that render its `as` prop.
 */
declare function Polymorphic<RenderProps>(props: RenderProps & PolymorphicAttributes<ValidComponent>): JSX.Element;
//#endregion
export { PolymorphicAttributes as a, Polymorphic as i, OverrideComponentProps as n, PolymorphicCallbackProps as o, OverrideProps as r, PolymorphicProps as s, ElementOf as t };