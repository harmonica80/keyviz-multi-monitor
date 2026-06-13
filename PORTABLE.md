# Portable Windows build

The portable build is a single executable and does not require an installer.

## Requirements

- Node.js
- Rust with the MSVC toolchain
- Microsoft C++ Build Tools
- WebView2 Runtime (included with current Windows 10 and Windows 11 systems)

## Build

```powershell
npm install
npm run build:portable
```

The executable is created at:

```text
src-tauri\target\release\keyviz.exe
```

Keyviz settings remain in the current user's application data directory. The
program files themselves are not installed or written to the Windows registry.
