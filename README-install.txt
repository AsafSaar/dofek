dofek v1.0 — Installation Notes
================================

Thanks for installing dofek.

  - Launch GUI:   Start Menu → "dofek"
  - Launch TUI:   open a terminal and run "dofek-tui" (or run dofek-tui.exe from this folder)
  - Full manual:  open "manual.html" in this folder (or Start Menu → "dofek Manual")
  - Press "?" inside either interface for the keybinding overlay.

Configuration
-------------
dofek looks for a config file in this order:
  1. --config <path>                    (TUI only)
  2. .\dofek.toml                       (current working directory)
  3. %APPDATA%\dofek\dofek.toml         (recommended)

See manual.html for the full option reference, or dofek.toml.example in the source repo.

Support
-------
  - Website:       https://dofek.dev
  - Source:        https://github.com/AsafSaar/dofek
  - Bugs:          https://github.com/AsafSaar/dofek/issues

MIT Licensed.
