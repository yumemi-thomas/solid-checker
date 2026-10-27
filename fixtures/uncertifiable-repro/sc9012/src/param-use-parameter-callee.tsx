// inferay ChatComposer/ComposerControls.tsx:24 (shared/lib/dom.tsx:106): a ref callback hands its element to a
// project function that passes it to a callable it was given as a parameter.
type RefTarget = ((element: HTMLElement) => void) | { current: HTMLElement | null };

function assignRef(target: RefTarget | undefined, element: HTMLElement): void {
  if (typeof target === "function") target(element);
  else if (target) target.current = element;
}

export function Controls(props: { controlsRef?: RefTarget }) {
  return <div ref={(element) => assignRef(props.controlsRef, element)} />;
}
