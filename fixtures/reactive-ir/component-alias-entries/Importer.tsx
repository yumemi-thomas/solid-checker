import { Imported as Renamed } from "./Exported";

const Local = Renamed;

export function Importer() {
  return <p><Local /></p>;
}
