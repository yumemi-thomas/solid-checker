import { createSignal } from "solid-js";

const [count] = createSignal(0);

function Title() {
  const value = count();
  return <b>{value}</b>;
}

function Body() {
  const value = count();
  return <i>{value}</i>;
}

// Positive: a nested namespace written inline, rendered as `<Card.Header.Title>`,
// and an anonymous member function. A data member (`id`) is never rendered.
export const Card = {
  id: "card",
  Header: { Title },
  Body,
  Footer: () => {
    const value = count();
    return <div>{value}</div>;
  },
};

export function Screen() {
  return (
    <Card.Body>
      <Card.Header.Title />
      <Card.Footer />
    </Card.Body>
  );
}
