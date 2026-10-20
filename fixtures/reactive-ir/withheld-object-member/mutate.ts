import { createSignal } from "solid-js";
import { api } from "./api";

// Imported for its side effect: replaces the literal's `read`.
const [count] = createSignal(0);
api.read = () => count();
