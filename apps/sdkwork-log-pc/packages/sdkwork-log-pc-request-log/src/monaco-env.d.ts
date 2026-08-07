// Vite `?worker` imports used by the Monaco (VS Code) editor wiring.
// Same pattern as the Cloud Router console config editor (vite/client types).
// Loaded explicitly via `/// <reference path="./monaco-env.d.ts" />` from
// components.tsx so every consuming package typecheck resolves the wildcard.

declare module '*?worker' {
  const workerConstructor: { new (): Worker };
  export default workerConstructor;
}
