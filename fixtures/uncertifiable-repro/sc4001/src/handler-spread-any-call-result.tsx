// viviana-ui-web apps/web/src/routes/solid-spectrum/docs/hooks/create-button.tsx:200: the props of a
// call whose callee has no declaration (a workspace package whose `dist` was never built, so its
// import resolves to nothing; a member of an `any` host object stands in for it here) are spread
// onto an element, and an `any` key set is no fact about the listeners the spread brings.
declare const aria: any;

export function GradientButton(props: { onPress?: () => void; children: string }) {
  const { buttonProps } = aria.createButton({ onPress: props.onPress });
  return <button {...buttonProps}>{props.children}</button>;
}
