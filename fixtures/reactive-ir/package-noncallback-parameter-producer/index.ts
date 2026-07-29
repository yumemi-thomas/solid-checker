declare function consumeValue(value: unknown): void;

export function passObject(props: { value: string }): void {
  consumeValue(props);
}
