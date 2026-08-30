# Browser end-to-end test

This suite builds the real WebAssembly app and drives the canvas through the
installed Microsoft Edge browser. It validates behavior from browser console
logs instead of screenshots.

Prerequisites:

- pnpm
- Trunk
- the `wasm32-unknown-unknown` Rust target
- Microsoft Edge at its standard Windows installation path

Run:

```powershell
pnpm --dir e2e install --frozen-lockfile
pnpm --dir e2e test
```

For iteration after the Web build is current:

```powershell
pnpm --dir e2e test:browser
```
