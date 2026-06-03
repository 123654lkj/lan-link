# lan-link-gui: cross-platform desktop client

A self-contained native desktop client for lan-link. Same `LanLinkClient`
protocol as `client_win.py` (the CLI), just with a Tk-based window.

## Why Tk

- Zero extra dependencies (tkinter ships with CPython on Windows, Linux,
  macOS).
- PyInstaller packages it to a single ~14 MB exe (we already use this for
  `client_win.py`).
- The wire protocol logic is shared with the CLI: no duplication.
- The user can later swap the UI for egui/Tauri/Slint without touching the
  network layer.

## Run

```
python client_gui.py
```

## Package a single exe (Windows)

```
pyinstaller --noconfirm --noconsole --onefile -n lan-link-gui --distpath dist-gui client_gui.py
```

The exe lives at `dist-gui/lan-link-gui.exe` (14.3 MB on Windows 11,
Python 3.11, PyInstaller 6.19).

## Package for Linux

Same PyInstaller command on a Linux host produces a self-contained ELF.
`apt install python3-tk` is required to build.

## Android

Not supported on tkinter. Plan: add a `crates/gui` eframe/egui binary as a
native Android shell, sharing the protocol crate with `lan-linkd`. The
`crates/gui` skeleton is already in the tree.

## Layout

```
client_gui.py
  App                 tk.Tk subclass, builds the window
  GuiClient           wraps LanLinkClient, posts events to a queue that
                      the UI drains via after()
  _build_layout       Top: host picker + Connect button
                      Left: quick commands, history, hosts editor
                      Center: monospace output (stdout/stderr/system tags)
                      Bottom: command entry
  _save_config        Persists to %APPDATA%/lan-link/gui-config.json
                      (or ~/.config/lan-link/gui-config.json on Linux)

Config schema (gui-config.json):
  {
    "hosts": [
      {"name": "tuanzi", "addr": "192.168.31.244:9876", "psk": "..."},
      ...
    ],
    "active": 0,
    "history": ["...", "..."]
  }
```

## Smoke test

`s-smoke_gui.py` runs the protocol layer against tuanzi without a window.
Output captured at 2026-06-01 17:28:

```
connected: True
running... waiting up to 10s
done in time, events:
  status: connected to 192.168.31.244:9876
  started
  chunk stream=0 data=b'Linux tuanzi 6.17.0-29-generic ...\n'
  done: exit=0
```
