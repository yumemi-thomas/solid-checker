import { createMemo, createSignal } from "solid-js";

const [userId] = createSignal(0);
declare function fetchPosts(): Promise<number[]>;
function byUser(posts: number[]) {
  return posts.filter((post) => post === userId());
}
function count(posts: number[]) {
  return posts.length;
}

export function AfterAwait() {
  // Control: the read written after the await is a violation.
  const direct = createMemo(async () => {
    const posts = await fetchPosts();
    return posts.filter((post) => post === userId());
  });
  // The read in a helper called after the await: uncertifiable.
  const helper = createMemo(async () => {
    const posts = await fetchPosts();
    return byUser(posts);
  });
  // A helper called before the await is tracked; one that reads nothing
  // reports nothing; a source read before the first suspension is already a
  // dependency (ADR 0195).
  const before = createMemo(async () => byUser(await fetchPosts()));
  const plain = createMemo(async () => count(await fetchPosts()));
  const tracked = createMemo(async () => {
    const id = userId();
    const posts = await fetchPosts();
    return byUser(posts).length + id;
  });
  return <div>{String(direct())}{String(helper())}{String(before())}{String(plain())}{String(tracked())}</div>;
}
