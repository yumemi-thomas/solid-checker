function Card(props: { active: boolean }) {
  return props.active ? <h1 /> : null;
}

export { Card };
