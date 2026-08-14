export function Panel(props: { duration: number }) {
  const createSomethingHandler = (delay: number) => () => {
    console.log(delay + props.duration);
  };
  const onSomething = createSomethingHandler(10);
  return <button onClick={onSomething}>Run</button>;
}
