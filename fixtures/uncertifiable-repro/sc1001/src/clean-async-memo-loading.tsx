// Clean. Mirrors helge-dev src/pages/Article.tsx:13-30: an async memo reading a prop before its await,
// read under Loading, with a non-keyed Show whose function child takes the accessor.
import { createMemo, Loading, Show } from "solid-js";

async function load(slug: string): Promise<{ title: string } | null> {
  return slug ? { title: `Post ${slug}` } : null;
}

function Article(props: { slug: string }) {
  const article = createMemo(async () => load(props.slug));
  return (
    <Loading fallback={<p>loading</p>}>
      <Show when={article()}>{(a) => <h1>{a().title}</h1>}</Show>
    </Loading>
  );
}

export function App() {
  return <Article slug="x" />;
}
