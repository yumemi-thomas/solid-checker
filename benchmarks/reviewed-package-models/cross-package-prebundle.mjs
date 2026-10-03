// Ask Vite to include the exact consumer imports in its normal dependency
// optimizer, including CommonJS packages. This changes no package source.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { ts } from './lower.mjs';
export default function crossPackagePrebundle() {
  return { name: 'experiment-cross-package-prebundle', transformed: [], refused: [],
    config(config) {
      const path = join(config.root, 'src/main.tsx');
      const source = ts.createSourceFile(path, readFileSync(path, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
      const include = source.statements.filter(ts.isImportDeclaration).map(node => node.moduleSpecifier.text);
      return { optimizeDeps: { include: [...new Set(include)] } };
    } };
}
