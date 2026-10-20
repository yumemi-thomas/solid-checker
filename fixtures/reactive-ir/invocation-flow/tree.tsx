type RowProps = (id: string) => Record<string, unknown>;

// `rowProps` reaches the row element's spread through `Level` and the
// recursive `Branch`, each forwarding it unchanged.
export function Tree(props: {
  ids: string[];
  rowProps?: RowProps;
  isProtected?: (id: string) => boolean;
  children: (id: string) => JSX.Element;
}) {
  return (
    <Level
      ids={props.ids}
      depth={0}
      rowProps={props.rowProps}
      isProtected={props.isProtected}
      children={props.children}
    />
  );
}

function Level(props: {
  ids: string[];
  depth: number;
  rowProps: RowProps | undefined;
  isProtected: ((id: string) => boolean) | undefined;
  children: (id: string) => JSX.Element;
}) {
  return (
    <div>
      {props.ids.map((id) => (
        <Branch id={id} depth={props.depth} rowProps={props.rowProps} isProtected={props.isProtected}>
          {props.children}
        </Branch>
      ))}
    </div>
  );
}

function Branch(props: {
  id: string;
  depth: number;
  rowProps: RowProps | undefined;
  isProtected: ((id: string) => boolean) | undefined;
  children: (id: string) => JSX.Element;
}) {
  const guarded = () => props.isProtected?.(props.id) === true;
  return (
    <div {...props.rowProps?.(props.id)} data-guarded={guarded()}>
      {props.children(props.id)}
      {props.depth < 2 ? (
        <Level
          ids={[`${props.id}-child`]}
          depth={props.depth + 1}
          rowProps={props.rowProps}
          isProtected={props.isProtected}
        >
          {props.children}
        </Level>
      ) : null}
    </div>
  );
}

// The same tree, except that the recursive element hands `label` a function
// that may use the props object it is called on as `this`.
export function HazardTree(props: { id: string; depth: number; rowProps?: RowProps; label?: () => string }) {
  const text = () => props.label?.() ?? "";
  return (
    <div {...props.rowProps?.(props.id)}>
      {text()}
      {props.depth < 2 ? (
        <HazardTree
          id={props.id}
          depth={props.depth + 1}
          rowProps={props.rowProps}
          label={function (this: unknown) {
            return String(this);
          }}
        />
      ) : null}
    </div>
  );
}
