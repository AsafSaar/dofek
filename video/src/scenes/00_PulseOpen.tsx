import React from "react";
import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { T, Format } from "../ui/Theme";

// Heartbeat path from website/favicon.svg, scaled
const PULSE_D = "M2 16 L8 16 L11 6 L15 26 L19 10 L22 16 L30 16";

export const PulseOpen: React.FC<{ format?: Format }> = ({ format = "hero" }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const dash = 200;
  const draw = interpolate(frame, [0, 50], [dash, 0], { extrapolateRight: "clamp" });
  const titleT = spring({ frame: frame - 35, fps, config: { damping: 18, stiffness: 110 } });
  const subT = spring({ frame: frame - 55, fps, config: { damping: 20, stiffness: 110 } });
  const fadeOut = interpolate(frame, [78, 90], [1, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });

  const isV = format === "vertical";

  return (
    <AbsoluteFill style={{ background: T.bg, fontFamily: T.font, opacity: fadeOut }}>
      <AbsoluteFill style={{ alignItems: "center", justifyContent: "center", flexDirection: "column", gap: 20 }}>
        <svg
          width={isV ? 520 : 640}
          height={isV ? 280 : 340}
          viewBox="0 0 32 32"
          style={{ filter: `drop-shadow(0 0 28px ${T.cpu}aa)` }}
        >
          <path
            d={PULSE_D}
            fill="none"
            stroke={T.cpu}
            strokeWidth={2.4}
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeDasharray={dash}
            strokeDashoffset={draw}
          />
        </svg>
        <div
          style={{
            opacity: titleT,
            transform: `translateY(${(1 - titleT) * 14}px)`,
            fontSize: isV ? 78 : 96,
            fontWeight: 800,
            color: T.text,
            letterSpacing: "0.18em",
            textTransform: "lowercase",
          }}
        >
          <span style={{ color: T.cpu }}>דּוֹפֶק</span>
          <span style={{ color: T.muted, margin: "0 18px" }}>·</span>
          <span>dofek</span>
        </div>
        <div
          style={{
            opacity: subT,
            color: T.dim,
            fontSize: isV ? 22 : 26,
            letterSpacing: "0.22em",
            textTransform: "uppercase",
          }}
        >
          /'do.fek/ — pulse
        </div>
      </AbsoluteFill>
    </AbsoluteFill>
  );
};
