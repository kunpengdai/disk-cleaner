# Disk Cleaner

**English** | [简体中文](README.zh-CN.md)

A Windows disk cleanup tool. Quick scan or full scan a drive, see every folder's size in a tree, and delete with confirmation steps that match each item's risk level.

Built with Rust + Tauri. Windows 10 / 11 (x64). The UI is available in Chinese and English.

## Download

Get the latest build from [Releases](https://github.com/kunpengdai/disk-cleaner/releases):

| File | Notes |
|---|---|
| `DiskCleaner_x.y.z_x64-setup.exe` | Installer |
| `DiskCleaner_x.y.z_x64_portable.exe` | Portable, double-click to run |

Requires WebView2, which Windows 11 includes. The program is not code-signed, so Windows may show "Windows protected your PC" the first time. Click **More info → Run anyway**.

## Features

- **Quick scan**: checks only known cleanable locations on the selected drive (temp files, browser caches, developer tool caches, WeChat / QQ / Feishu caches, Steam caches, the Recycle Bin and more). Usually takes a few seconds.
- **Full scan**: measures every folder on the drive and shows them in a tree sorted by size.
- **Four risk levels** decide how much confirmation a delete needs (see below).
- **No silent permanent deletes**: when moving to the Recycle Bin, Windows' "too big for the Recycle Bin, delete permanently?" warning is kept.
- **Delete log**: every delete is recorded in `%LOCALAPPDATA%\DiskCleaner\delete-log.jsonl`.
- **Chinese / English**: switch with the button in the top-right corner. It follows the system language on first launch and remembers your choice.

## Risk levels

| Level | Confirmation | How it is deleted |
|---|---|---|
| Safe | Confirm once | Can be deleted permanently (default) |
| Confirm | Confirm once, with the full list shown | Recycle Bin |
| Caution, explicitly recognized by a rule (app data, personal files, chat history, etc., or folders that contain them) | Review impact → tick "I understand" → type the name by hand (paste blocked) + 5-second countdown; checked again by the backend | Recycle Bin only |
| Caution, only because no rule recognizes it | Review impact → tick "I understand"; checked again by the backend | Recycle Bin only |
| Protected | Cannot be deleted; a safe alternative is shown | — |

## Build

```powershell
npm install
npm run tauri dev                    # development mode
npx tauri build --no-bundle          # builds src-tauri\target\release\disk-cleaner.exe
npx tauri build                      # builds the NSIS installer
```

## Tests

```powershell
cd src-tauri
cargo test                                                  # rules, risk levels, confirmation checks, translations, real deletes (only of files the test creates)
cargo test --release real_machine -- --ignored --nocapture  # a real quick scan of C: and D: plus a full scan of C: on this PC (read-only)
```

## Code layout

| File | Purpose |
|---|---|
| `src-tauri/rules/builtin.json` | Cleanup rules (paths, risk level, Chinese text). Add new rules here |
| `src-tauri/rules/builtin.en.json` | English names and notes for the rules, keyed by rule ID |
| `src-tauri/src/rules.rs` | Built-in protection list, rule loading and matching |
| `src-tauri/src/scanner.rs` | Multi-threaded scanner (does not follow links; cloud-only files count as 0) |
| `src-tauri/src/tree.rs` | Result tree: risk classification (top-down), size totals (bottom-up) |
| `src-tauri/src/delete.rs` | Delete plan, backend confirmation checks, deleting, delete log |
| `src-tauri/src/winutil.rs` | Drive list, administrator rights, Recycle Bin |
| `src-tauri/src/i18n.rs` | Backend language switch |
| `src/i18n.ts` | UI text in Chinese and English |
| `src/` | UI (tree, details, cleanable summary, confirmation dialogs) |

When you add a rule, add its English text to `builtin.en.json` too. `cargo test` fails if any rule is missing a translation.

The design document is [DESIGN.md](DESIGN.md) (in Chinese).
