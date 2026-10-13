import { createSignal } from "solid-js";
import { useLocation, useParams, defineRoutes } from "@solidjs/router";

// Correct consumer: all returned reactive fields are read in tracked JSX.
export function Clean() {
  const location = useLocation();
  const params = useParams();
  return <p>{location.search}{params.id}</p>;
}

// Proven violation under the integration premise: one-time signal seed.
export function Seed() {
  const location = useLocation();
  const [search] = createSignal(location.search);
  return <p>{search()}</p>;
}

// This helper is independent of the premise and must retain its claim.
export const routes = defineRoutes([{ path: "/" }]);
