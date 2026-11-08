import { shared } from "../../listeners";
export function App() { setTimeout(() => window.dispatchEvent(new Event("click")), 0); return <button onClick={shared} />; }
