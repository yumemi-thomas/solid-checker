import { render } from "@solidjs/web";
import { translate } from "./source";

function Labelled(props: { label: string; k: string }) {
  return (
    <p>
      {props.label}
      {props.k}
    </p>
  );
}

function Page(props: { k: string }) {
  return <Labelled label={translate(() => "x")} k={props.k} />;
}

render(() => <Page k="a" />, document.body);
