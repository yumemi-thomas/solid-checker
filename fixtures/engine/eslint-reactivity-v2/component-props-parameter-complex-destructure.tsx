function Aliased({ title: heading }: { title: string }) {
  return <h1>{heading}</h1>;
}

function Defaulted({ title = "Untitled" }: { title?: string }) {
  return <h1>{title}</h1>;
}

function WithRest({ title, ...rest }: { title: string; id: string }) {
  return <h1 id={rest.id}>{title}</h1>;
}

export { Aliased, Defaulted, WithRest };
