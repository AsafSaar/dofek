import React from "react";
import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { T, Format } from "../ui/Theme";

export const Problem: React.FC<{ format?: Format }> = ({ format = "hero" }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const isV = format === "vertical";

  const mockT = spring({ frame, fps, config: { damping: 18, stiffness: 90 } });
  const strikeProgress = interpolate(frame, [70, 130], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  const textT = spring({ frame: frame - 20, fps, config: { damping: 20, stiffness: 100 } });

  const rows = [
    { name: "System Idle Process", cpu: 91 },
    { name: "Registry", cpu: 0 },
    { name: "explorer.exe", cpu: 1 },
    { name: "svchost.exe", cpu: 0 },
    { name: "python.exe", cpu: 12 },
    { name: "ollama.exe", cpu: 47 },
  ];

  return (
    <AbsoluteFill
      style={{
        background: T.bg,
        fontFamily: T.font,
        padding: isV ? 60 : 100,
        flexDirection: isV ? "column" : "row",
        alignItems: "center",
        justifyContent: "center",
        gap: 60,
      }}
    >
      <div
        style={{
          flex: isV ? "0 0 auto" : "0 0 46%",
          opacity: mockT,
          transform: `translateX(${(1 - mockT) * -40}px)`,
          width: isV ? "85%" : undefined,
          background: "#1a1a1a",
          border: `1px solid #333`,
          borderRadius: 6,
          overflow: "hidden",
          filter: "saturate(0.2) brightness(0.7)",
          position: "relative",
        }}
      >
        <div
          style={{
            background: "#2b2b2b",
            color: "#aaa",
            padding: "8px 14px",
            fontSize: 13,
            fontFamily: "system-ui",
            borderBottom: "1px solid #333",
          }}
        >
          Task Manager
        </div>
        <div style={{ padding: 14 }}>
          {rows.map((r, i) => (
            <div
              key={i}
              style={{
                display: "grid",
                gridTemplateColumns: "1fr 80px",
                padding: "8px 0",
                borderBottom: "1px solid #2a2a2a",
                color: "#ccc",
                fontFamily: "system-ui",
                fontSize: 13,
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
            left: 20,
            height: 4,
            width: `calc(${strikeProgress * 100}% - 40px)`,
            background: T.crit,
            transformOrigin: "left center",
            borderRadius: 2,
            boxShadow: `0 0 14px ${T.crit}`,
          }}
        />
      </div>
      <div
        style={{
          flex: isV ? "0 0 auto" : 1,
          opacity: textT,
          transform: `translateY(${(1 - textT) * 14}px)`,
          maxWidth: isV ? "90%" : 560,
          textAlign: isV ? "center" : "left",
        }}
      >
        <div
          style={{
            fontSize: isV ? 44 : 52,
            fontWeight: 700,
            color: T.text,
            lineHeight: 1.2,
            letterSpacing: "-0.01em",
          }}
        >
          Task Manager wasn't built for what your machine does today.
        </div>
      </div>
    </AbsoluteFill>
  );
};
