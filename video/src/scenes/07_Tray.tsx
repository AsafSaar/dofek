import React from "react";
import { AbsoluteFill, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { T, Format } from "../ui/Theme";

const trayBars = (frame: number, count: number) =>
  Array.from({ length: count }).map((_, i) => {
    const v = 16 + 11 * Math.sin((i + frame * 0.6) * 0.5) + 5 * Math.cos(i * 0.9 + frame * 0.15);
    return Math.max(3, v);
  });

export const Tray: React.FC<{ format?: Format }> = ({ format = "hero" }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const isV = format === "vertical";

  const headT = spring({ frame, fps, config: { damping: 20, stiffness: 110 } });
  const zoomT = spring({ frame: frame - 8, fps, config: { damping: 18, stiffness: 100 } });
  const arrowT = spring({ frame: frame - 30, fps, config: { damping: 22, stiffness: 130 } });

  const smallBars = trayBars(frame, 16);
  const bigBars = trayBars(frame, 28);

  return (
    <AbsoluteFill style={{ background: T.bg, fontFamily: T.font, display: "flex", flexDirection: "column" }}>
      {/* simulated menu bar — full-bleed at top edge */}
      <div
        style={{
          background: "rgba(255,255,255,0.04)",
          borderBottom: `1px solid ${T.bdr2}`,
          padding: "14px 32px",
          display: "flex",
          alignItems: "center",
          gap: 22,
          color: T.dim,
          fontSize: 16,
        }}
      >
        <span style={{ fontSize: 16 }}></span>
        <span>File</span>
        <span>Edit</span>
        <span>View</span>
        <span>Window</span>
        <span>Help</span>
        <div style={{ flex: 1 }} />
        <span style={{ fontSize: 14 }}>Wi-Fi</span>
        <span style={{ fontSize: 14 }}>🔋 87%</span>
        <div
          id="tray-icon"
          style={{
            display: "flex",
            alignItems: "center",
            gap: 8,
            padding: "4px 10px",
            borderRadius: 4,
            background: `${T.cpu}10`,
            border: `1px solid ${T.cpu}55`,
            transform: `scale(${0.95 + zoomT * 0.05})`,
            boxShadow: `0 0 16px ${T.cpu}55`,
          }}
        >
          <span style={{ display: "flex", alignItems: "flex-end", gap: 1.5, height: 22 }}>
            {smallBars.map((h, i) => (
              <div key={i} style={{ width: 3, height: `${h * 1.1}px`, background: T.cpu, opacity: 0.9 }} />
            ))}
          </span>
          <span style={{ color: T.cpu, fontSize: 14, fontWeight: 800 }}>34%</span>
        </div>
        <span style={{ fontSize: 14 }}>10:42</span>
      </div>

      {/* main stage */}
      <div
        style={{
          flex: 1,
          padding: isV ? 40 : 80,
          display: "flex",
          flexDirection: isV ? "column" : "row",
          alignItems: "center",
          gap: isV ? 40 : 80,
          minHeight: 0,
        }}
      >
        {/* left side: text */}
        <div
          style={{
            flex: 1,
            opacity: headT,
            transform: `translateY(${(1 - headT) * 14}px)`,
            display: "flex",
            flexDirection: "column",
            gap: 26,
            justifyContent: "center",
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
            System tray companion
          </div>
          <div
            style={{
              fontSize: isV ? 60 : 92,
              fontWeight: 800,
              color: T.text,
              letterSpacing: "-0.02em",
              lineHeight: 1.05,
            }}
          >
            Sparkline in your <span style={{ color: T.cpu }}>tray</span>.
          </div>
          <div style={{ color: T.dim, fontSize: isV ? 24 : 32, lineHeight: 1.4, maxWidth: 600 }}>
            Always there. Never in the way.
          </div>
          <div style={{ color: T.muted, fontSize: 16, lineHeight: 1.6, maxWidth: 600 }}>
            Live CPU sparkline rendered into the icon itself. Click to open the full app — or close the window and let it fade into the menu bar.
          </div>
        </div>

        {/* right side: zoomed callout connecting back to the tray icon */}
        <div
          style={{
            flex: 1,
            opacity: zoomT,
            transform: `scale(${0.85 + zoomT * 0.15})`,
            position: "relative",
            display: "flex",
            justifyContent: "center",
            alignItems: "center",
          }}
        >
          {/* dashed line from upper-right (toward menu bar) */}
          <svg
            width="100%"
            height="60"
            style={{
              position: "absolute",
              top: -40,
              right: 40,
              opacity: arrowT * 0.5,
              pointerEvents: "none",
            }}
          >
            <path
              d="M 80 50 Q 200 -20 400 -10"
              stroke={T.cpu}
              strokeWidth={2}
              strokeDasharray="6,6"
              fill="none"
              opacity={0.6}
            />
          </svg>

          <div
            style={{
              background: T.surf,
              border: `1px solid ${T.cpu}55`,
              borderRadius: 14,
              padding: "32px 48px",
              display: "flex",
              alignItems: "center",
              gap: 36,
              boxShadow: `0 0 80px ${T.cpu}44`,
            }}
          >
            <span style={{ display: "flex", alignItems: "flex-end", gap: 5, height: 140 }}>
              {bigBars.map((h, i) => (
                <div
                  key={i}
                  style={{
                    width: 11,
                    height: `${h * 5.5}px`,
                    background: `linear-gradient(180deg, ${T.cpu}, ${T.cpu}88)`,
                    opacity: 0.9,
                    borderRadius: 2,
                  }}
                />
              ))}
            </span>
            <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
              <div style={{ color: T.muted, fontSize: 14, letterSpacing: "0.18em", fontWeight: 600 }}>CPU · LIVE</div>
              <div style={{ color: T.cpu, fontSize: 96, fontWeight: 900, lineHeight: 1 }}>34%</div>
              <div style={{ color: T.dim, fontSize: 14, letterSpacing: "0.06em" }}>updated every 1s</div>
            </div>
          </div>
        </div>
      </div>
    </AbsoluteFill>
  );
};
