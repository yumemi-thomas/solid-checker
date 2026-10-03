import {For as Each, Show} from 'solid-js';
import * as Solid from 'solid-js';

const Registry={Local:(props:{value:number})=><span>{props.value}</span>};
function Shadow(){
  const For=(props:{value:number})=><span>{props.value}</span>;
  return <For value={1}/>;
}

export function Cases(){
  const label='日本語';
  return <>
    <span>{label}</span>
    <Each each={[{id:1}]}>{item=><span>{item.id}</span>}</Each>
    <Solid.For each={[1]}>{item=><span>{item}</span>}</Solid.For>
    <Show when={true}><span>visible</span></Show>
    <Registry.Local value={1}/>
    <Shadow/>
  </>;
}
