# Dofek Launch Video — Remotion Plan

## Context

Dofek is in a celebratory moment: v1.5 ships a fully cross-platform build (Windows, Linux, macOS Apple Silicon), three first-party plugins (`dofek-ollama`, `dofek-docker`, `dofek-net-ping`), a managed plugin store with hot reload, system-tray companion with live CPU sparkline, and a trading-terminal UX with candlestick CPU charts and AI workload detection. There is no existing marketing video — only static screenshots in `website/` and `docs/assets/`. We want a Remotion-driven launch video to push on X, LinkedIn, and YouTube/Shorts.

**Outcome:** two cuts produced from one Remotion project — a 60s 16:9 hero spot and a 30s 9:16 vertical short. Pure Remotion recreations of the UI (no screen capture), text + licensed music, faithful to the trading-terminal palette.

---

## 1. Project setup

New isolated subdirectory so it doesn't pollute the Rust workspace:

```
/Users/asaf/dev/dofek/video/
  package.json              # remotion + react + typescript
  remotion.config.ts
  tsconfig.json
  src/
    Root.tsx                # registers compositions
    compositions/
      HeroSpot60.tsx        # 1920x1080, 60s @ 30fps
      VerticalShort30.tsx   # 1080x1920, 30s @ 30fps
    scenes/                 # shared scene components, reused by both cuts
      00_PulseOpen.tsx
      01_Problem.tsx
      02_Candlestick.tsx
      03_AIAware.tsx
      04_CrossPlatform.tsx
      05_DualUI.tsx
      06_Plugins.tsx
      07_Tray.tsx
      08_CTA.tsx
    ui/                     # reusable Dofek-look components
      Theme.ts              # palette + font tokens (mirrors gui/frontend)
      Panel.tsx             # bordered surface (var(--surf), var(--bdr))
      TickerBar.tsx         # top ticker recreation
      Candlestick.tsx       # animated wick + IQR body widget
      ProcessRow.tsx        # AI/DEV/WATCH coloured row
      AreaChart.tsx         # filled area for GPU/MEM/NET
      MetricPill.tsx        # ticker pill
    audio/
      track.mp3             # licensed bed (Artlist / Epidemic / Pixabay) — user supplies
    assets/
      favicon.svg           # symlink or copy of website/favicon.svg
      JetBrainsMono-*.ttf   # webfont (loadFont via @remotion/google-fonts)
  out/                      # rendered MP4s — gitignored
```

`.gitignore` additions: `video/node_modules`, `video/out`, `video/audio/track.mp3` (license-restricted bed).

`package.json` scripts:

```
"build:hero":    "remotion render HeroSpot60 out/dofek-hero-60s.mp4 --codec=h264"
"build:short":   "remotion render VerticalShort30 out/dofek-short-30s.mp4 --codec=h264"
"build:both":    "npm run build:hero && npm run build:short"
"studio":        "remotion studio"
```

## 2. Theme token file (drop-in port)

`src/ui/Theme.ts` mirrors the CSS variables in `gui/frontend/index.html` so visuals match the app exactly:

```ts
export const T = {
  bg:'#060810', surf:'#0b1120', bdr:'#182035', bdr2:'#1f2d48',
  text:'#e2e8f0', dim:'#94a3b8', muted:'#3d5070',
  cpu:'#38bdf8', gpu:'#a78bfa', mem:'#34d399',
  netUp:'#fb923c', netDn:'#38bdf8',
  ai:'#c084fc', dev:'#60a5fa', watch:'#fbbf24',
  warn:'#fbbf24', crit:'#f87171',
  font:`'JetBrains Mono','Cascadia Code','Fira Code',Consolas,monospace`,
};
```

Load JetBrains Mono via `@remotion/google-fonts/JetBrainsMono`.

## 3. Hero cut — 60s @ 30fps (1920×1080), scene-by-scene

| # | Frames | Duration | Scene | On-screen text | Visual |
|---|--------|----------|-------|----------------|--------|
| 0 | 0–90 | 0–3s | PulseOpen | `דּוֹפֶק · DOFEK` + "/'do.fek/ — pulse" | Heartbeat waveform (animated favicon path stroke-dashoffset) on `bg`, sky-blue glow |
| 1 | 90–270 | 3–9s | Problem | "Task Manager wasn't built for what your machine does today." | Greyed-out generic task-manager mockup, then strike-through animation |
| 2 | 270–540 | 9–18s | Candlestick | "See **variance**, not just averages." sub: "wick = min/max · body = P25–P75" | Animated candlestick chart drawing in left-to-right, sky-blue wicks growing per frame, IQR bodies fading in. Use spring() per candle |
| 3 | 540–780 | 18–26s | AIAware | "AI-aware. Per-process VRAM. Inference state." | Process rows streaming in with violet `●` AI badges, VRAM column animating, an "ollama — inferring" pill pulsing in ticker |
| 4 | 780–960 | 26–32s | CrossPlatform | "Windows · Linux · macOS" | Three stacked window chromes (Win/GTK/macOS traffic lights), each fading in showing a different pane of dofek. OS logos as outlines |
| 5 | 960–1140 | 32–38s | DualUI | "TUI for the terminal. Tauri GUI for the desktop." | Split-screen: left = ratatui terminal box (monospace half-blocks), right = GUI window with same data — synchronized animation proves they share a core |
| 6 | 1140–1410 | 38–47s | Plugins | "Plugins. JSON over stdio. Hot-reload." Three cards: `dofek-ollama` / `dofek-docker` / `dofek-net-ping` | Cards fly in from right, each shows the live data style they emit (model list, container chips, latency sparkline). Sub: "Build your own. Ship in 30 lines." |
| 7 | 1410–1560 | 47–52s | Tray | "Sparkline in your tray. Always there. Never in the way." | Top-right macOS menu-bar / Windows tray icon close-up with live CPU sparkline updating |
| 8 | 1560–1800 | 52–60s | CTA | `dofek.dev` · `github.com/AsafSaar/...` · `brew install dofek` | All three platform install commands stacked, pulse waveform underline, end card holds 1.5s |

Music bed: 60s electronic/synthwave with a subtle 4-on-the-floor pulse — beat lines up with candle drops in scene 2.

## 4. Vertical short — 30s @ 30fps (1080×1920)

Reuses scene components with a `format="vertical"` prop that the components honour to re-stack panels vertically. Trimmed scene list:

- 0–3s: PulseOpen
- 3–10s: Candlestick (vertical full-bleed)
- 10–17s: AIAware (process rows tall list)
- 17–23s: Plugins (stacked cards)
- 23–30s: CTA

Same audio track, lower-third captions baked in for muted autoplay (TikTok/Reels default).

## 5. Reusable UI components — what to mirror from the app

To keep visuals identical to the product, these Remotion components mirror existing structure (no need to invent — copy spacing/colours from these files):

- **`Candlestick.tsx`** — mirror logic in `src/ui/candlestick.rs` (OHLC layout, half-block trick translates to thin SVG wicks + filled rect bodies). Reference: `src/ui/sparkline_buf.rs` `CandleBuf` for OHLC semantics.
- **`TickerBar.tsx`** — mirror `src/ui/ticker.rs` and the `.ticker` block in `gui/frontend/index.html` (hostname + clock + metric pills).
- **`ProcessRow.tsx`** — mirror `.proc-row` styles in `gui/frontend/index.html` (lines 198–217), especially the left-border accent for AI/DEV/WATCH (purple/blue/amber).
- **`AreaChart.tsx`** — mirror `src/ui/area_chart.rs` (filled, multi-series, threshold line).
- **`MetricPill.tsx`** — mirror the `.t-pill` style.

Copy the actual spacing/font-size values from `gui/frontend/index.html` so the Remotion render is screenshot-indistinguishable from the live GUI.

## 6. Copy lockup (final wording)

Pulled from the existing website voice (`website/index.html`) so brand stays consistent:

- Tagline: **"System monitor for the AI era."**
- Hero line: **"Your workstation deserves better than Task Manager."**
- CTA: **"dofek.dev"** with `brew install dofek` / `winget install dofek` / `apt install dofek` rotating.
- Closing kicker: **"דּוֹפֶק — pulse."** with the heartbeat waveform.

## 7. Files to create / modify

**New:**
- `video/package.json`, `video/remotion.config.ts`, `video/tsconfig.json`
- `video/src/Root.tsx`
- `video/src/compositions/HeroSpot60.tsx`, `video/src/compositions/VerticalShort30.tsx`
- `video/src/scenes/0[0-8]_*.tsx` (9 scene files)
- `video/src/ui/{Theme,Panel,TickerBar,Candlestick,ProcessRow,AreaChart,MetricPill}.tsx`
- `video/.gitignore`

**Modified:**
- Root `.gitignore` — add `video/node_modules`, `video/out`, `video/audio/track.mp3`
- `README.md` — add a one-line "Promo video" link near the screenshots section once rendered

**Untouched:** the Rust workspace, `gui/`, `website/`, plugin crates. Zero risk to the shipping app.

## 8. Verification

End-to-end check before declaring done:

1. `cd video && npm install` — completes cleanly.
2. `npm run studio` — Remotion Studio opens, both compositions render in the preview, scrubbing the timeline shows every scene with no missing-asset warnings.
3. Visual parity check: open `gui/frontend/index.html` in a browser, screenshot the ticker + a process row, paste alongside the Remotion render at the same frame — colours and spacing should match within a couple of px.
4. `npm run build:hero` — produces `out/dofek-hero-60s.mp4`, ~1080p H.264, under 30 MB, plays in QuickTime.
5. `npm run build:short` — produces `out/dofek-short-30s.mp4`, 1080×1920, plays in QuickTime, captions readable on a phone-sized preview.
6. Spot-check audio sync: candlestick beats land on the music's downbeats in scene 2.
7. Drag the hero MP4 into X's composer / LinkedIn's uploader to confirm aspect + duration are accepted without re-encode warnings.

## 9. Out of scope (explicitly)

- Music licensing / track selection — user supplies `video/audio/track.mp3`.
- AI voiceover (deferred per chosen direction).
- Real screen recordings (deferred per chosen direction).
- Posting / scheduling on social platforms.
- Translations / localised cuts beyond English.
