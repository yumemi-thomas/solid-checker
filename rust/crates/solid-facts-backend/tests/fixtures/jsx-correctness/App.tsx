const renderHeader = () => <header>Header</header>;
const Header = () => <header>Header</header>;
const renderValue = () => 42;
// A dynamic href: Solid removes the attribute when the expression is nullish,
// and an `a` without an href is not draggable by default — so the anchor's
// default cannot be proven and `draggable={false}` on it stays clean.
const link = (): string | undefined => undefined;

export function Panel() {
  return (
    <article>
      {renderHeader()}
      <Header />
      {renderValue()}
      <img draggable />
      <img draggable="true" />
      <img draggable={true} />
      <img draggable={false} />
      <a href="/download" draggable={false}>save</a>
      <a draggable={false}>plain</a>
      <a href={link()} draggable={false}>dynamic</a>
      <div draggable={false} />
      <img draggable="false" />
    </article>
  );
}
