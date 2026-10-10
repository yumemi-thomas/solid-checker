// Exact JSX symbol resolution. A checked overload is selected by TypeScript,
// and must belong to the resolved symbol; unchecked implementations stay open.
export function byteSpan(source,node){
  return {start:Buffer.byteLength(source.text.slice(0,node.getStart(source)),'utf8'),end:Buffer.byteLength(source.text.slice(0,node.end),'utf8')};
}

export function resolveJsxDeclaration(ts,checker,opening,{checkedApplication}){
  let symbol=checker.getSymbolAtLocation(opening.tagName);
  const visited=new Set();
  while(symbol&&(symbol.flags&ts.SymbolFlags.Alias)){
    if(visited.has(symbol))return {status:'open',reason:'alias-cycle'};
    visited.add(symbol);symbol=checker.getAliasedSymbol(symbol);
  }
  const declarations=symbol?.declarations??[];
  if(declarations.length===1)return {status:'exact',declaration:declarations[0],declarations,kind:'single-declaration',symbol};
  if(!checkedApplication){
    // This locates the declared function family without selecting an overload.
    // Merged namespaces, values and inferred callable properties remain open.
    const declaredFamily=declarations.length>1&&declarations.every(node=>
      ts.isFunctionDeclaration(node)&&node.getSourceFile().isDeclarationFile&&
      node.getSourceFile()===declarations[0].getSourceFile()&&!node.body);
    if(declaredFamily)return {status:'exact',declaration:null,declarations,kind:'declared-overload-family',symbol};
    return {status:'open',reason:'declaration-not-unique'};
  }
  const signature=checker.getResolvedSignature(opening),declaration=signature?.declaration;
  if(!declaration||!declarations.includes(declaration))return {status:'open',reason:'declaration-not-unique'};
  return {status:'exact',declaration,declarations,kind:'checked-overload-signature',symbol};
}
