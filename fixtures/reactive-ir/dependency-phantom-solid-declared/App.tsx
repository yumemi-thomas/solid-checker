import { Router, useLocation } from "dep-router";

function Read() {
  const location = useLocation();
  return <p>{location.search}</p>;
}
export function App() { return <Router><Read /></Router>; }
