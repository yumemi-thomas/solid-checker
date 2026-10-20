export function ForwardedHelper(props: { value: string }) {
  const browse = () => props.value;
  const text = browse(); // Reactive witness survives the exported forward.
  return <p>{text}</p>;
}
