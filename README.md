# NSCB Desktop

Modern desktop GUI for Nintendo Switch game file operations: compress, decompress, merge, convert, split, repack, and rename (NSP, XCI, NSZ, XCZ, NCZ).

Built with Tauri v2 + React + Vite. Powered by [nscb_rust](https://github.com/cxfcxf/nscb_rust).

## Features

- Compress NSP/XCI to NSZ/XCZ (zstd level 1-22)
- Decompress NSZ/XCZ/NCZ back to NSP/XCI
- Merge, downgrade, or repack into one NSP/XCI (with firmware controls: RSV cap, key generation patch)
- Convert between NSP and XCI
- Split multi-title files into per-title folders or individual NSP/XCI files
- Create/Repack NSP from split folders
- Rename files using embedded metadata + NUTDB lookup (with rename mode, language, version, DLC controls)
- NUTDB utilities: refresh cache, look up title IDs
- View file info (content details and metadata summary)
- Drag & drop files or folders (rename supports recursive folder input)
- Live output console + progress tracking
- Batch support for compress/decompress/convert/split/rename
- Dark theme UI with SVG iconography and file type badges
- Auto-download nscb_rust backend from GitHub releases, or import manually
- First-launch setup wizard for encryption keys
- Cross-platform: Windows x86_64, Linux x86_64, macOS ARM64

## Supported Platforms

| Platform | Architecture | Backend Binary |
|---|---|---|
| Windows | x86_64 | `nscb_rust.exe` |
| Linux | x86_64 | `nscb_rust-linux-amd64` |
| macOS | ARM64 | `nscb_rust-macos-arm64` |

## Prerequisites

- Switch keys file (`prod.keys` or `keys.txt`)
- nscb_rust backend binary (auto-downloaded or imported via Settings)
- **Windows**: WebView2 runtime (included in Windows 10/11)
- **Linux**: WebKitGTK 4.1, libayatana-appindicator3
- **macOS**: macOS 11.0+

## Usage

1. Run the app directly (standalone portable exe), or use the platform installer.
2. On first launch, the setup wizard prompts you to import **encryption keys** (`prod.keys` or `keys.txt`).
3. If the nscb_rust backend is missing, a banner directs you to **Settings > Tools** where you can download it from GitHub or import manually.
4. Pick an operation from the sidebar and drop your files.

## Development

Requires [Rust](https://rustup.rs) + [Node.js](https://nodejs.org) (v18+).

```bash
npm install
npm run dev        # Tauri dev mode (hot reload)
npm run dev:vite   # Vite dev server only (no Tauri)
npm test           # Run unit tests
```

## Build

```bash
# NSIS installer (Windows) / deb (Linux) / dmg (macOS)
npm run build

# Portable folder at release/NSCB Desktop/
npm run dist:portable
```

## Project Layout

```text
nscb-desktop/
|- ui/                     # Frontend (React + TypeScript)
|  |- App.tsx              # All components + pages
|  |- App.css              # Dark theme design system
|  |- main.tsx             # Entry point
|  `- lib/
|     |- api.ts            # Tauri plugin wrappers
|     |- nscb-runner.ts    # Backend process + progress parsing
|     `- nscb-runner.test.ts # Unit tests for CLI arg building
|- src-tauri/              # Tauri v2 / Rust backend
|  |- src/lib.rs           # Commands: run_nscb, import_keys, etc.
|  |- tauri.conf.json      # Window config, bundling
|  `- capabilities/        # Security permissions
`- scripts/portable.mjs    # Portable folder assembly
```

## License

ISC
