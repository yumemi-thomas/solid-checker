// Reduced from installed Vite types/importMeta.d.ts. These six signatures and
// the env field are byte-faithful; unrelated hot/glob APIs are unused here.
interface ImportMetaEnv extends Record<string, any> {
  BASE_URL: string
  MODE: string
  DEV: boolean
  PROD: boolean
  SSR: boolean
}
interface ImportMeta {
  readonly env: ImportMetaEnv
}
