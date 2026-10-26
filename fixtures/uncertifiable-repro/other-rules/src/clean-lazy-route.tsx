// Clean. Mirrors viviana-ui-web apps/web/src/routes/admin.tsx:10-12 and solid-groove src/router.tsx:42: a
// lazy component from a dynamic import (of this module's own export), rendered under Loading.
import { lazy, Loading } from "solid-js";

export function Admin(props: { title: string }) {
  return <h1>{props.title}</h1>;
}

const AdminPage = lazy(() => import("./clean-lazy-route").then((mod) => ({ default: mod.Admin })));

export function Route() {
  return (
    <Loading fallback={<p>loading admin</p>}>
      <AdminPage title="admin" />
    </Loading>
  );
}
