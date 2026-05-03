import React from "react";
import { AbsoluteFill, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { T, Format } from "../ui/Theme";

const useBars = (count: number) => {
  const frame = useCurrentFrame();
  return Array.from({ length: count }).map((_, i) => {
    const v = 45 + 30 * Math.sin((i + frame * 0.4) * 0.45) + 14 * Math.cos(i * 0.7 + frame * 0.1);
    return Math.max(8, Math.min(95, v));
  });
};

const ProcLine: React.FC<{ name: string; cpu: string; color?: string; dim?: boolean }> = ({ name, cpu, color, dim }) => (
  <div style={{ display: "flex", justifyContent: "space-between", fontSize: 17, color: dim ? T.muted : T.text }}>
    <span>{name}</span>
    <span style={{ color: color ?? T.cpu, fontWeight: 600 }}>{cpu}</span>
  </div>
);

const TUIPane: React.FC = () => {
  const bars = useBars(54);
  return (
    <div
      style={{
        background: "#000",
        border: `1px solid ${T.bdr2}`,
        borderRadius: 6,
        padding: 28,
        fontFamily: T.font,
        color: T.cpu,
        fontSize: 16,
        height: "100%",
        display: "flex",
        flexDirection: "column",
        gap: 14,
        boxShadow: `0 30px 60px rgba(0,0,0,.5)`,
      }}
    >
      <div style={{ color: T.muted, letterSpacing: "0.1em", fontSize: 15 }}>$ dofek-tui</div>
      <div style={{ borderBottom: `1px solid ${T.bdr}`, paddingBottom: 12, display: "flex", gap: 18, fontSize: 16 }}>
        <span style={{ color: T.cpu, fontWeight: 800, letterSpacing: "0.08em" }}>dofek</span>
        <span style={{ color: T.cpu }}>CPU 47%</span>
        <span style={{ color: T.gpu }}>GPU 71%</span>
        <span style={{ color: T.mem }}>MEM 58%</span>
        <span style={{ color: T.ai }}>AI ollama·inf</span>
      </div>
      <div style={{ flex: 1, display: "flex", flexDirection: "column", gap: 14, minHeight: 0 }}>
        <div style={{ flex: 1.4, display: "flex", alignItems: "flex-end", gap: 3, minHeight: 0 }}>
          {bars.map((h, i) => (
            <div
              key={i}
              style={{
                flex: 1,
                height: `${h}%`,
                background: T.cpu,
                opacity: 0.9,
              }}
            />
          ))}
        </div>
        <div style={{ borderTop: `1px solid ${T.bdr}`, paddingTop: 12, display: "flex", flexDirection: "column", gap: 8 }}>
          <ProcLine name="ollama          [4821]" cpu="62.1%" color={T.ai} />
          <ProcLine name="python          [7733]" cpu="41.7%" color={T.ai} />
          <ProcLine name="cargo           [2210]" cpu="18.4%" color={T.dev} />
          <ProcLine name="code            [1144]" cpu="12.3%" color={T.dev} dim />
        </div>
      </div>
      <div style={{ color: T.dim, fontSize: 14, letterSpacing: "0.04em", borderTop: `1px solid ${T.bdr}`, paddingTop: 10 }}>
        [q]uit  [tab] panel  [c]pu  [g]pu  [m]em  [n]et  [p]rocs  [?] help
      </div>
    </div>
  );
};

const GUIPane: React.FC = () => {
  const bars = useBars(48);
  return (
    <div
      style={{
        background: T.surf,
        border: `1px solid ${T.bdr2}`,
        borderRadius: 6,
        fontFamily: T.font,
        height: "100%",
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
        boxShadow: `0 30px 60px rgba(0,0,0,.5)`,
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 14,
          padding: "14px 22px",
          borderBottom: `1px solid ${T.bdr2}`,
          background: T.surf2,
        }}
      >
        <span style={{ color: T.cpu, fontWeight: 800, letterSpacing: "0.1em", fontSize: 18 }}>dofek</span>
        <span style={{ color: T.dim, fontSize: 13, marginLeft: 8 }}>CPU</span>
        <span style={{ color: T.cpu, fontWeight: 700, fontSize: 15 }}>47%</span>
        <span style={{ color: T.dim, fontSize: 13 }}>GPU</span>
        <span style={{ color: T.gpu, fontWeight: 700, fontSize: 15 }}>71%</span>
        <span style={{ color: T.dim, fontSize: 13 }}>MEM</span>
        <span style={{ color: T.mem, fontWeight: 700, fontSize: 15 }}>58%</span>
        <div style={{ flex: 1 }} />
        <span
          style={{
            padding: "3px 10px",
            border: "1px solid rgba(192,132,252,.4)",
            background: "rgba(192,132,252,.1)",
            color: T.ai,
            borderRadius: 3,
            fontSize: 12,
            letterSpacing: "0.08em",
          }}
        >
          AI ollama · inferring
        </span>
      </div>
      <div style={{ flex: 1, padding: 22, display: "flex", flexDirection: "column", gap: 18, minHeight: 0 }}>
        <div style={{ flex: 1.4, display: "flex", alignItems: "flex-end", gap: 4, minHeight: 0 }}>
          {bars.map((h, i) => (
            <div
              key={i}
              style={{
                flex: 1,
                height: `${h}%`,
                background: `linear-gradient(180deg, ${T.cpu}, ${T.cpu}66)`,
                borderRadius: 2,
              }}
            />
          ))}
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 9, borderTop: `1px solid ${T.bdr}`, paddingTop: 14 }}>
          <ProcLine name="ollama" cpu="62.1%" color={T.ai} />
          <ProcLine name="python (torch)" cpu="41.7%" color={T.ai} />
          <ProcLine name="cargo" cpu="18.4%" color={T.dev} />
          <ProcLine name="code (Cursor)" cpu="12.3%" color={T.dev} dim />
        </div>
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
    <AbsoluteFill
      style={{
        background: T.bg,
        fontFamily: T.font,
        padding: isV ? "50px 40px 60px" : "60px 80px 70px",
        display: "flex",
        flexDirection: "column",
        gap: isV ? 26 : 36,
      }}
    >
      <div
        style={{
          opacity: headT,
          transform: `translateY(${(1 - headT) * 14}px)`,
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          gap: 12,
        }}
      >
        <div
          style={{
            color: T.muted,
            fontSize: isV ? 18 : 20,
            letterSpacing: "0.32em",
            textTransform: "uppercase",
            fontWeight: 600,
          }}
        >
          One core. Two surfaces.
        </div>
        <div
          style={{
            fontSize: isV ? 56 : 88,
            fontWeight: 800,
            color: T.text,
            letterSpacing: "-0.02em",
            textAlign: "center",
            lineHeight: 1.05,
          }}
        >
          <span style={{ color: T.cpu }}>TUI</span> for the terminal.
          <span style={{ margin: "0 18px", color: T.muted }}>·</span>
          <span style={{ color: T.gpu }}>GUI</span> for the desktop.
        </div>
      </div>
      <div
        style={{
          flex: 1,
          display: "flex",
          flexDirection: isV ? "column" : "row",
          gap: isV ? 24 : 32,
          minHeight: 0,
        }}
      >
        <div style={{ flex: 1, opacity: lT, transform: `translateX(${(1 - lT) * -24}px)`, minHeight: 0 }}>
          <TUIPane />
        </div>
        <div style={{ flex: 1, opacity: rT, transform: `translateX(${(1 - rT) * 24}px)`, minHeight: 0 }}>
          <GUIPane />
        </div>
      </div>
    </AbsoluteFill>
  );
};
