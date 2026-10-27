type Press = { onPress?: () => void };

// The prop reaches a DOM event as the handler itself.
export function Direct(props: Press) {
  return <button onClick={props.onPress} />;
}

// The prop is called inside the element's own handler.
export function InHandler(props: Press) {
  return <button onClick={() => props.onPress?.()} />;
}
