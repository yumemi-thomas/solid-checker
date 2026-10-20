import { createMemo, createSignal } from "solid-js";

// reactive-read-after-await (SC1002): every reactive input is read before the
// first await, or the post-await dependency is its own synchronous memo.
declare function fetchPosts(): Promise<{ author: number }[]>;

export function Posts() {
  const [userId] = createSignal(0);
  const profile = createMemo(async () => {
    const id = userId();
    const posts = await fetchPosts();
    return posts.filter((post) => post.author === id);
  });
  const posts = createMemo(() => fetchPosts());
  const byAuthor = createMemo(() => posts().filter((post) => post.author === userId()));
  return (
    <div>
      {profile().length}
      {byAuthor().length}
    </div>
  );
}
