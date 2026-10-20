import { merge, omit, untrack } from "solid-js";
import { Dynamic, type ValidComponent } from "@solidjs/web";

// The shape of `@kobalte/core@2.0.0-alpha.0`'s `Polymorphic` and `ButtonRoot`,
// and of a project `Button` over them: each hop narrows or widens its props
// only through `omit`, `merge` and spreads, and the last renders
// `<Dynamic {...others} component={props.as} />`.

export type PolymorphicProps = {
  as?: ValidComponent;
  onClick?: (event: MouseEvent) => void;
  tooltip?: () => string;
  children?: string;
  [attribute: string]: unknown;
};

export function Polymorphic(props: PolymorphicProps) {
  const others = omit(props, "as");
  if (!untrack(() => props.as)) throw new Error("Polymorphic is missing the required `as` prop.");
  return <Dynamic {...others} component={props.as} />;
}

// `ButtonRoot`: `as="button"` before the spread, so a caller's `as` wins.
export function ButtonRoot(props: PolymorphicProps) {
  const mergedProps = merge({ type: "button" }, props);
  const others = omit(mergedProps, "ref", "type", "disabled");
  return <Polymorphic as="button" type={mergedProps.type} {...others} />;
}

// A project button over it, like openbot's.
export function Button(props: PolymorphicProps & { variant?: string }) {
  const others = omit(props, "variant", "class", "children");
  const rootProps = others as PolymorphicProps;
  return (
    <ButtonRoot class={props.variant ?? "default"} {...rootProps}>
      {props.children}
    </ButtonRoot>
  );
}

// A wrapper of that button, like openbot's `IconButton`.
export function IconButton(props: PolymorphicProps & { label: string }) {
  const others = omit(props, "label", "children");
  return (
    <Button aria-label={props.label} {...others}>
      {props.children}
    </Button>
  );
}

// `component` written after an unknown spread names the element itself.
export function Anchor(props: PolymorphicProps) {
  return <Dynamic {...props} component="a" />;
}

// A getter in the merged literal: its value is whatever it returns when read.
export function GetterRoot(props: PolymorphicProps) {
  const mergedProps = merge(
    {
      get as(): ValidComponent {
        return "button";
      },
    },
    props,
  );
  return <Polymorphic {...mergedProps} />;
}

// Invokes the handler while it renders.
export function CallingRoot(props: PolymorphicProps) {
  props.onClick?.(new MouseEvent("click"));
  const others = omit(props, "as");
  return <Dynamic {...others} component="button" />;
}
