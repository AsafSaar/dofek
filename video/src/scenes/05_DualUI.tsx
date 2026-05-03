import React from "react";
import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { T, Format } from "../ui/Theme";

const TUIPane: React.FC = () => {
  const frame = useCurrentFrame();
  // ratatui-style half-block "candles" using box-drawing chars
  const cols = 36;
  const blocks = Array.from({ length: cols }).map((_, i) => {
    const v = 30 + 25 * Math.sin((i + frame * 0.4) * 0.45) + 12 * Math.cos(i * 0.7 + frame * 0.1);
    return Math.max(8, Math.min(55, v));
  });
  return (
    <div
      style={{
        background: "#000",
        border: `1px solid ${T.bdr2}`,
        borderRadius: 4,
        padding: 16,
        fontFamily: T.font,
        color: T.cpu,
        fontSize: 13,
        height: "100%",
        display: "flex",
        flexDirection: "column",
        gap: 8,
      }}
    >
      <div style={{ color: T.muted, letterSpacing: "0.1em" }}>$ dofek-tui</div>
      <div style={{ color: T.text, borderBottom: `1px solid ${T.bdr}`, paddingBottom: 6 }}>
        <span style={{ color: T.cpu, fontWeight: 700 }}>dofek</span>
        <span style={{ color: T.muted, marginLeft: 14 }}>CPU 47% · GPU 71% · MEM 58%</span>
      </div>
      <div style={{ display: "flex", alignItems: "flex-end", gap: 2, height: 200 }}>
        {blocks.map((h, i) => (
          <div key={i} style={{ width: 10, height: h * 3, background: T.cpu, opacity: 0.85 }} />
        ))}
      </div>
      <div style={{ color: T.dim, marginTop: "auto" }}>
        [q]uit  [tab] panel  [c]pu  [g]pu  [m]em  [n]et
      </div>
    </div>
  );
};

const GUIPane: React.FC = () => {
  const frame = useCurrentFrame();
  const cols = 36;
  const blocks = Array.from({ length: cols }).map((_, i) => {
    const v = 30 + 25 * Math.sin((i + frame * 0.4) * 0.45) + 12 * Math.cos(i * 0.7 + frame * 0.1);
    return Math.max(8, Math.min(55, v));
  });
  return (
    <div
      style={{
        background: T.surf,
        border: `1px solid ${T.bdr2}`,
        borderRadius: 4,
        padding: 16,
        fontFamily: T.font,
        height: "100%",
        display: "flex",
        flexDirection: "column",
        gap: 10,
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 10,
          paddingBottom: 8,
          borderBottom: `1px solid ${T.bdr2}`,
        }}
      >
        <span style={{ color: T.cpu, fontWeight: 800, letterSpacing: "0.1em" }}>dofek</span>
        <span style={{ color: T.dim, fontSize: 11 }}>CPU</span>
        <span style={{ color: T.cpu, fontWeight: 700 }}>47%</span>
        <span style={{ color: T.dim, fontSize: 11, marginLeft: 8 }}>GPU</span>
        <span style={{ color: T.gpu, fontWeight: 700 }}>71%</span>
      </div>
      <div style={{ flex: 1, display: "flex", alignItems: "flex-end", gap: 3 }}>
        {blocks.map((h, i) => (
          <div
            key={i}
            style={{
              width: 12,
              height: h * 3,
              background: `linear-gradient(180deg, ${T.cpu}, ${T.cpu}88)`,
              borderRadius: 2,
            }}
          />
        ))}
      </div>
    </div>
  );
};

export const DualUI: React.FC<{ format?: Format }> = ({ format = "hero" }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const isV = format === "vertical";

  const headT = spring({ frame, fps, config: { damping: 20, stiffness: 110 } });
  const lT = spring({ frame: frame - 6, fps, config: { damping: 22, stiffness: 130 } });
  const rT = spring({ frame: frame - 14, fps, config: { damping: 22, stiffness: 130 } });

  return (
    <AbsoluteFill style={{ background: T.bg, fontFamily: T.font, padding: isV ? 40 : 80 }}>
      <div
        style={{
          opacity: headT,
          transform: `translateY(${(1 - headT) * 12}px)`,
          fontSize: isV ? 44 : 56,
          fontWeight: 700,
          color: T.text,
          textAlign: "center",
          marginBottom: 36,
        }}
      >
        <span style={{ color: T.cpu }}>TUI</span> for the terminal.
        <span style={{ margin: "0 14px", color: T.muted }}>·</span>
        <span style={{ color: T.gpu }}>GUI</span> for the desktop.
      </div>
      <div
        style={{
          flex: 1,
          display: "flex",
          flexDirection: isV ? "column" : "row",
          gap: 24,
          minHeight: 0,
        }}
      >
        <div style={{ flex: 1, opacity: lT, transform: `translateX(${(1 - lT) * -24}px)` }}>
          <TUIPane />
        </div>
        <div style={{ flex: 1, opacity: rT, transform: `translateX(${(1 - rT) * 24}px)` }}>
          <GUIPane />
        </div>
      </div>
    </AbsoluteFill>
  );
};
