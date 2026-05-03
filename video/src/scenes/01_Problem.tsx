import React from "react";
import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { T, Format } from "../ui/Theme";

const ROWS = [
  { name: "System Idle Process", cpu: 91 },
  { name: "Registry", cpu: 0 },
  { name: "explorer.exe", cpu: 1 },
  { name: "svchost.exe", cpu: 0 },
  { name: "RuntimeBroker.exe", cpu: 0 },
  { name: "MsMpEng.exe", cpu: 4 },
  { name: "SearchIndexer.exe", cpu: 0 },
  { name: "python.exe", cpu: 12 },
  { name: "ollama.exe", cpu: 47 },
  { name: "WerFault.exe", cpu: 0 },
  { name: "spoolsv.exe", cpu: 0 },
  { name: "audiodg.exe", cpu: 0 },
];

export const Problem: React.FC<{ format?: Format }> = ({ format = "hero" }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const isV = format === "vertical";

  const mockT = spring({ frame, fps, config: { damping: 18, stiffness: 90 } });
  const strikeProgress = interpolate(frame, [70, 130], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  const textT = spring({ frame: frame - 18, fps, config: { damping: 20, stiffness: 100 } });

  return (
    <AbsoluteFill
      style={{
        background: T.bg,
        fontFamily: T.font,
        padding: isV ? 50 : 80,
        flexDirection: isV ? "column" : "row",
        alignItems: "stretch",
        justifyContent: "space-between",
        gap: isV ? 40 : 70,
      }}
    >
      <div
        style={{
          flex: 1,
          opacity: mockT,
          transform: `translateX(${(1 - mockT) * -40}px)`,
          background: "#1c1c1c",
          border: `1px solid #333`,
          borderRadius: 8,
          overflow: "hidden",
          filter: "saturate(0.15) brightness(0.78)",
          position: "relative",
          display: "flex",
          flexDirection: "column",
          boxShadow: "0 30px 60px rgba(0,0,0,.55)",
        }}
      >
        <div
          style={{
            background: "#2b2b2b",
            color: "#bbb",
            padding: "14px 22px",
            fontSize: 18,
            fontFamily: "system-ui, -apple-system, Segoe UI, sans-serif",
            borderBottom: "1px solid #333",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
          }}
        >
          <span>Task Manager</span>
          <span style={{ color: "#777", fontSize: 14, letterSpacing: "0.5em" }}>—  ▢  ✕</span>
        </div>
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "1fr 100px",
            padding: "10px 22px",
            color: "#888",
            fontFamily: "system-ui, -apple-system, Segoe UI, sans-serif",
            fontSize: 13,
            letterSpacing: "0.06em",
            textTransform: "uppercase",
            borderBottom: "1px solid #333",
          }}
        >
          <span>Name</span>
          <span style={{ textAlign: "right" }}>CPU</span>
        </div>
        <div style={{ flex: 1, padding: "0 22px" }}>
          {ROWS.map((r, i) => (
            <div
              key={i}
              style={{
                display: "grid",
                gridTemplateColumns: "1fr 100px",
                padding: "14px 0",
                borderBottom: "1px solid #2a2a2a",
                color: "#ccc",
                fontFamily: "system-ui, -apple-system, Segoe UI, sans-serif",
                fontSize: 18,
              }}
            >
              <span>{r.name}</span>
              <span style={{ textAlign: "right", color: "#888" }}>{r.cpu}%</span>
            </div>
          ))}
        </div>
        {/* strike-through */}
        <div
          style={{
            position: "absolute",
            top: "50%",
            left: 30,
            height: 6,
            width: `calc(${strikeProgress * 100}% - 60px)`,
            background: T.crit,
            borderRadius: 3,
            boxShadow: `0 0 22px ${T.crit}`,
          }}
        />
      </div>

      <div
        style={{
          flex: 1,
          opacity: textT,
          transform: `translateY(${(1 - textT) * 14}px)`,
          display: "flex",
          flexDirection: "column",
          justifyContent: "center",
          gap: 30,
        }}
      >
        <div
          style={{
            color: T.muted,
            fontSize: isV ? 20 : 22,
            letterSpacing: "0.32em",
            textTransform: "uppercase",
            fontWeight: 600,
          }}
        >
          The problem
        </div>
        <div
          style={{
            fontSize: isV ? 64 : 92,
            fontWeight: 800,
            color: T.text,
            lineHeight: 1.05,
            letterSpacing: "-0.02em",
          }}
        >
          Task Manager wasn't built for what your{" "}
          <span style={{ color: T.cpu }}>machine</span> does today.
        </div>
        <div
          style={{
            color: T.dim,
            fontSize: isV ? 22 : 28,
            lineHeight: 1.4,
            maxWidth: 720,
          }}
        >
          Generic process lists. No GPU. No VRAM. No idea what your AI workloads are doing.
        </div>
      </div>
    </AbsoluteFill>
  );
};
