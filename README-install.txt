dofek v1.3 — Installation Notes
================================

Thanks for installing dofek.

Launching
---------
Windows
  - GUI:    Start Menu → "dofek"
  - TUI:    open a terminal and run "dofek-tui" (or run dofek-tui.exe from
            this folder)
  - Manual: open "manual.html" in this folder (or Start Menu → "dofek Manual")

Linux
  - GUI:    application menu → "dofek"  (or run "dofek-gui" from a terminal)
  - TUI:    "dofek-tui" from any terminal (installed to /usr/bin by .deb/.rpm;
            extract from the .AppImage if needed)
  - Manual: open "manual.html" from /usr/share/dofek/ or this folder

macOS (Apple Silicon)
  - GUI:    Launchpad → "dofek", or "open /Applications/dofek.app"
            First launch shows "dofek.app is damaged and can't be opened" —
            the app isn't damaged, it's unsigned and quarantined by your
            browser. Strip the flag once and it launches normally:
              xattr -dr com.apple.quarantine /Applications/dofek.app
            (macOS 14 Sonoma and earlier also accept right-click → Open
            → Open; macOS 15 Sequoia removed that bypass.)
  - TUI:    "dofek-tui" from Terminal.app or iTerm2 (chmod +x first if
            you downloaded the standalone binary)
  - Manual: open "manual.html" from inside dofek.app/Contents/Resources/

Press "?" inside either interface for the keybinding overlay.

Configuration
-------------
dofek looks for a config file in this order:
  1. --config <path>                       (TUI only)
  2. ./dofek.toml                          (current working directory)
  3. user config directory:
       Windows: %APPDATA%\dofek\dofek.toml
       Linux:   ~/.config/dofek/dofek.toml
       macOS:   ~/Library/Application Support/dofek/dofek.toml

See manual.html for the full option reference, or dofek.toml.example in the
source repo.

Support
-------
  - Website:  https://dofek.dev
  - Source:   https://github.com/AsafSaar/dofek
  - Bugs:     https://github.com/AsafSaar/dofek/issues

MIT Licensed.
