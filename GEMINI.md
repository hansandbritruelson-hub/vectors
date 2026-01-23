# GEMINI Rust Engine Build Instructions

To build the Rust WASM engine and generate bindings for the web project, use:

```bash
npm run engine:build
```

## Troubleshooting Build Issues
If you encounter permission errors or "Operation not permitted" on macOS, it is likely due to quarantine flags on the local toolchain. The build script `scripts/build-engine.sh` handles this automatically by:
1. Setting `RUSTUP_HOME` and `CARGO_HOME` to the local project directories (`.rustup_home` and `.cargo_home`).
2. Prioritizing the local toolchain in the `PATH`.
3. Running `xattr -rd com.apple.quarantine` on the local toolchain directories.

The generated artifacts are stored in `web/src/pkg/`.
