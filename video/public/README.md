# video/public

Remotion's `staticFile()` resolves paths from this directory.

## Required for music

Drop your licensed audio bed here as `track.mp3`. Both compositions
load it via `staticFile("track.mp3")` and slice it to their exact
length (`endAt={durationInFrames}`), so a longer track is fine —
only the first 60s (hero) or 30s (vertical short) will be used.

If `track.mp3` is missing, render will fail with
`Error: Cannot find module …/track.mp3`. To temporarily disable
music, pass `withAudio={false}` on the composition or flip the
default in `src/Root.tsx`.

This file is git-tracked so the directory exists; `track.mp3`
itself is gitignored (license-restricted).
