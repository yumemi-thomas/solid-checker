// app-game gpu-text-rendering DocumentRendererProvider.tsx:64 (createTypeGpuRenderer.ts:22): a component hands
// a document to a project function that passes it to another project function from inside a closure.
type Doc = { title: string; pages: string[] };

function render(document: Doc): string {
  return `${document.title}:${document.pages.length}`;
}

async function prepare(document: Doc): Promise<string> {
  const draw = () => render(document);
  await Promise.resolve();
  return draw();
}

export function Viewer() {
  const document: Doc = { title: "a", pages: ["b"] };
  const prepared = prepare(document);
  return <button onClick={() => void prepared}>open</button>;
}
