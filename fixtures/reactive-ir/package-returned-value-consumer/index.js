function useConsumer() {
  return value => value.name;
}

export function consumeObject(props) {
  const consume = useConsumer();
  return consume(props);
}
