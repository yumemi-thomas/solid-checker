const renderHeader = () => <header>Header</header>;
const Header = () => <header>Header</header>;
const renderValue = () => 42;

export function Panel() {
  return (
    <article>
      {renderHeader()}
      <Header />
      {renderValue()}
      <img draggable />
      <img draggable="true" />
      <img draggable={true} />
    </article>
  );
}
