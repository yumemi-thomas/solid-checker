// solid-groove LibraryFacets.tsx:33-35: an exported component destructures its props in the body.
type Browser = { filter: () => string };

export function FacetBar(props: { browser: Browser }) {
  const { browser } = props;
  return <p>{browser.filter()}</p>;
}
