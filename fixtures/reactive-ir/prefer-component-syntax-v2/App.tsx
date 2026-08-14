import { renderCard, renderValue } from "./source";

export function View() {
  return (
    <section>
      {renderCard(true)}
      {renderValue()}
    </section>
  );
}

export function Shadowed() {
  const renderCard = () => 42;
  return <div>{renderCard()}</div>;
}
