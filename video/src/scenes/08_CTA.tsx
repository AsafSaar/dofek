import React from "react";
import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { T, Format } from "../ui/Theme";

const PULSE_D = "M2 16 L8 16 L11 6 L15 26 L19 10 L22 16 L30 16";

const InstallLine: React.FC<{ os: string; cmd: string; appearAt: number }> = ({ os, cmd, appearAt }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const t = spring({ frame: frame - appearAt, fps, config: { damping: 22, stiffness: 130 } });
  return (
    <div
      style={{
        opacity: t,
        transform: `translateY(${(1 - t) * 14}px)`,
        display: "flex",
        alignItems: "center",
        gap: 18,
        background: T.surf,
        border: `1px solid ${T.bdr2}`,
        borderRadius: 4,
        padding: "14px 22px",
        fontFamily: T.font,
      }}
    >
      <span
        style={{
          color: T.muted,
          fontSize: 11,
          letterSpacing: "0.12em",
          textTransform: "uppercase",
          minWidth: 80,
        }}
      >
        {os}
      </span>
      <span style={{ color: T.cpu, fontSize: 22, fontWeight: 600 }}>$</span>
      <span style={{ color: T.text, fontSize: 22 }}>{cmd}</span>
    </div>
  );
};

export const CTA: React.FC<{ format?: Format }> = ({ format = "hero" }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const isV = format === "vertical";

  const taglineT = spring({ frame, fps, config: { damping: 20, stiffness: 110 } });
  const dash = 200;
  const draw = interpolate(frame, [10, 70], [dash, 0], { extrapolateRight: "clamp" });
  const urlT = spring({ frame: frame - 60, fps, config: { damping: 22, stiffness: 130 } });

  return (
    <AbsoluteFill
      style={{
        background: T.bg,
        fontFamily: T.font,
        alignItems: "center",
        justifyContent: "center",
        padding: isV ? 40 : 80,
      }}
    >
      <div
        style={{
          opacity: taglineT,
          transform: `translateY(${(1 - taglineT) * 12}px)`,
          fontSize: isV ? 38 : 46,
          color: T.dim,
          textAlign: "center",
          letterSpacing: "0.02em",
        }}
      >
        System monitor for the <span style={{ color: T.ai }}>AI era</span>.
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 14, marginTop: 50, width: isV ? "92%" : 620 }}>
        <InstallLine os="macOS" cmd="brew install dofek" appearAt={6} />
        <InstallLine os="Windows" cmd="winget install dofek" appearAt={20} />
        <InstallLine os="Linux" cmd="apt install dofek" appearAt={34} />
      </div>

      <div style={{ marginTop: 60, opacity: urlT, textAlign: "center" }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "center", gap: 18 }}>
          <svg width={56} height={56} viewBox="0 0 32 32">
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
          <span style={{ fontSize: isV ? 56 : 68, fontWeight: 800, color: T.text, letterSpacing: "0.04em" }}>
            dofek<span style={{ color: T.cpu }}>.dev</span>
          </span>
        </div>
        <div
          style={{
            marginTop: 18,
            color: T.muted,
            fontSize: isV ? 18 : 20,
            letterSpacing: "0.18em",
          }}
        >
          דּוֹפֶק — pulse.
        </div>
      </div>
    </AbsoluteFill>
  );
};
