// Exact imported component with an exclusively event-bound local arrow.
export function NamedImported(props: { onPress: () => void }) {
  const handler = () => props.onPress();
  return <button onClick={handler} />;
}
