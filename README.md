# Gemini Vectors

A high-performance vector graphics and image processing engine built with Rust (WASM) and Vue 3.

## Features

- **Vector Graphics**: Draw shapes, bezier paths, and text with full control over fill, stroke, and gradients.
- **Image Processing**:
  - AI-powered background removal.
  - Image vectorization (tracing).
  - Magic wand selection.
  - Clone stamp and eraser tools.
- **File Support**: Import and export PSD, AI, and SVG files.
- **Responsive UI**: Built with Vue 3 and Tailwind CSS, featuring a professional-grade canvas with rulers, guides, and infinite zoom.
- **Undo/Redo**: Full history support for all operations.

## Project Structure

- `engine/`: Rust source code for the graphics engine.
- `web/`: Vue 3 frontend application.
- `electron/`: Electron wrapper for desktop support.
- `scripts/`: Build and utility scripts.

## Getting Started

### Prerequisites

- Node.js and npm
- Rust toolchain (cargo, rustup)
- `wasm-pack` (for building the engine)

### Building the Engine

To build the Rust WASM engine:

```bash
npm run engine:build
```

### Running the Web App

```bash
cd web
npm install
npm run dev
```

### Running the Electron App

```bash
npm install
npm run electron:dev
```

## Architecture

The project uses a hybrid architecture where the core graphics logic is implemented in Rust for performance and safety, then compiled to WASM. The frontend communicates with the Rust engine via a JSON-based command system.

### Command System

Commands are sent from TypeScript to Rust in the following format:

```json
{
  "action": "add",
  "params": {
    "type": "Rectangle",
    "x": 100,
    "y": 100,
    "width": 200,
    "height": 150,
    "fill": "#4facfe"
  }
}
```

## License

MIT