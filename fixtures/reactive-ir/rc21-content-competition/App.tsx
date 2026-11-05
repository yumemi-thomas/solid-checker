export function Proven(props: { html: string; text: string }) {
  return (
    <>
      <div innerHTML={props.html}>child</div>
      <div innerText={props.text}>child</div>
      <div textContent="t">{/* comment */}</div>
      <div innerHTML="<b>x</b>" textContent="t" />
    </>
  );
}

export function Blank() {
  return (
    <>
      <div innerHTML="<b>x</b>">{null}</div>
      <div innerHTML="<b>x</b>">
      </div>
      <div innerHTML="<b>x</b>" />
    </>
  );
}

export function Unproven(props: { html?: string; text?: string; child?: string }) {
  return (
    <>
      <div textContent={props.text}>child</div>
      <div innerHTML={props.html} textContent="t" />
      <div innerHTML="<b>x</b>">{props.child}</div>
    </>
  );
}

const closed = { title: "t" };
const content = { innerHTML: "<b>x</b>" };
export function Spreads(props: { any: any }) {
  return (
    <>
      <div {...closed}>child</div>
      <div {...{ title: "t" }}>child</div>
      <div {...content}>child</div>
      <div {...props.any}>child</div>
      <div {...props.any} />
      <div {...content} {...closed} />
      <p title="t" />
    </>
  );
}
