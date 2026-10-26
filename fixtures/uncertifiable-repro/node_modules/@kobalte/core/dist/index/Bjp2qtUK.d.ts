import { s as PolymorphicProps, t as ElementOf } from "./CFIwL2Gi.js";
import { ValidComponent } from "@solidjs/web";
import { Ref } from "solid-js";
//#region src/button/button-root.d.ts
interface ButtonRootOptions {}
interface ButtonRootCommonProps<T extends HTMLElement = HTMLElement> {
  /** Whether the button is disabled. */
  disabled: boolean | undefined;
  type: string | undefined;
  ref: Ref<T>;
  tabindex: number | string | undefined;
}
interface ButtonRootRenderProps extends ButtonRootCommonProps {
  role: "menuitem" | "button" | undefined;
  "aria-disabled": "true" | undefined;
  "data-disabled": string | undefined;
}
type ButtonRootProps<T extends ValidComponent | HTMLElement = HTMLElement> = ButtonRootOptions & Partial<ButtonRootCommonProps<ElementOf<T>>>;
/**
 * Button enables users to trigger an action or event, such as submitting a form,
 * opening a dialog, canceling an action, or performing a delete operation.
 * This component is based on the [WAI-ARIA Button Pattern](https://www.w3.org/WAI/ARIA/apg/patterns/button/)
 */
declare function ButtonRoot<T extends ValidComponent = "button">(props: PolymorphicProps<T, ButtonRootProps<T>>): import("@solidjs/web").JSX.Element;
declare namespace index_d_exports {
  export { Button, ButtonRootCommonProps, ButtonRootOptions, ButtonRootProps, ButtonRootRenderProps, ButtonRoot as Root };
}
declare const Button: typeof ButtonRoot;
//#endregion
export { ButtonRootOptions as a, ButtonRootCommonProps as i, index_d_exports as n, ButtonRootProps as o, ButtonRoot as r, ButtonRootRenderProps as s, Button as t };