import React from "react";
import { AbsoluteFill, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { T, Format } from "../ui/Theme";

const trayBars = (frame: number) =>
  Array.from({ length: 20 }).map((_, i) => {
    const v = 12 + 9 * Math.sin((i + frame * 0.6) * 0.5) + 4 * Math.cos(i * 0.9 + frame * 0.15);
    return Math.max(2, v);
  });

export const Tray: React.FC<{ format?: Format }> = ({ format = "hero" }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const isV = format === "vertical";

  const headT = spring({ frame, fps, config: { damping: 20, stiffness: 110 } });
  const zoomT = spring({ frame: frame - 10, fps, config: { damping: 18, stiffness: 100 } });

  const bars = trayBars(frame);

  return (
    <AbsoluteFill style={{ background: T.bg, fontFamily: T.font, padding: isV ? 40 : 90 }}>
      {/* simulated menu bar at top */}
      <div
        style={{
          background: "rgba(255,255,255,0.04)",
          border: `1px solid ${T.bdr2}`,
          borderRadius: 4,
          padding: "8px 14px",
          display: "flex",
          alignItems: "center",
          gap: 16,
          color: T.dim,
          fontSize: 13,
        }}
      >
        <span></span>
        <span>File</span>
        <span>Edit</span>
        <span>View</span>
        <div style={{ flex: 1 }} />
        <span style={{ fontSize: 11 }}>Wi-Fi</span>
        <span style={{ fontSize: 11 }}>🔋 87%</span>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 6,
            padding: "2px 6px",
            borderRadius: 3,
            background: `${T.cpu}10`,
            border: `1px solid ${T.cpu}40`,
            transform: `scale(${0.95 + zoomT * 0.05})`,
          }}
        >
          <span style={{ display: "flex", alignItems: "flex-end", gap: 1, height: 16 }}>
            {bars.slice(0, 14).map((h, i) => (
              <div key={i} style={{ width: 2, height: `${h * 0.9}px`, background: T.cpu, opacity: 0.85 }} />
            ))}
          </span>
          <span style={{ color: T.cpu, fontSize: 11, fontWeight: 700 }}>34%</span>
        </div>
        <span style={{ fontSize: 11 }}>10:42</span>
      </div>

      {/* enlarged callout */}
      <div
        style={{
          marginTop: 60,
          alignSelf: "center",
          opacity: zoomT,
          transform: `scale(${0.85 + zoomT * 0.15})`,
        }}
      >
        <div
          style={{
            background: T.surf,
            border: `1px solid ${T.cpu}50`,
            borderRadius: 10,
            padding: "20px 32px",
            display: "flex",
            alignItems: "center",
            gap: 28,
            boxShadow: `0 0 60px ${T.cpu}33`,
          }}
        >
          <span style={{ display: "flex", alignItems: "flex-end", gap: 4, height: 80 }}>
            {bars.map((h, i) => (
              <div
                key={i}
                style={{
                  width: 8,
                  height: `${h * 4}px`,
                  background: T.cpu,
                  opacity: 0.85,
                  borderRadius: 2,
                }}
              />
            ))}
          </span>
          <div>
            <div style={{ color: T.muted, fontSize: 11, letterSpacing: "0.12em" }}>CPU · LIVE</div>
            <div style={{ color: T.cpu, fontSize: 56, fontWeight: 800 }}>34%</div>
          </div>
        </div>
      </div>

      <div
        style={{
          opacity: headT,
          transform: `translateY(${(1 - headT) * 12}px)`,
          textAlign: "center",
          fontSize: isV ? 38 : 48,
          fontWeight: 700,
          color: T.text,
          marginTop: 60,
          lineHeight: 1.25,
        }}
      >
        Sparkline in your tray.<br />
        <span style={{ color: T.dim, fontSize: isV ? 22 : 28, fontWeight: 500 }}>
          Always there. Never in the way.
        </span>
      </div>
    </AbsoluteFill>
  );
};
