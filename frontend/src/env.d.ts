
declare module '*.vue' {
  import type { DefineComponent } from 'vue'
  const component: DefineComponent<{}, {}, any>
  export default component
}

// wasm 二进制以 ?url 形式导入（vite 资产管线返回发布 URL）。
declare module '*.wasm?url' {
  const src: string
  export default src
}

/// <reference types="vite/client" />
