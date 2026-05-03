import React from "react";
import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { T, Format } from "../ui/Theme";

const PULSE_D = "M2 16 L8 16 L11 6 L15 26 L19 10 L22 16 L30 16";

type RowKind = "command" | "platform";

const InstallRow: React.FC<{
  os: string;
  detail: string;
  accent: string;
  kind: RowKind;
  appearAt: number;
}> = ({ os, detail, accent, kind, appearAt }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const t = spring({ frame: frame - appearAt, fps, config: { damping: 22, stiffness: 130 } });
  return (
    <div
      style={{
        opacity: t,
        transform: `translateY(${(1 - t) * 16}px)`,
        display: "flex",
        alignItems: "center",
        gap: 28,
        background: T.surf,
        border: `1px solid ${T.bdr2}`,
        borderRadius: 6,
        padding: "22px 32px",
        fontFamily: T.font,
        boxShadow: `0 14px 36px rgba(0,0,0,.45)`,
      }}
    >
      <span
        style={{
          color: accent,
          fontSize: 14,
          letterSpacing: "0.22em",
          textTransform: "uppercase",
          minWidth: 110,
          fontWeight: 800,
        }}
      >
        {os}
      </span>
      <span style={{ color: accent, fontSize: 26, fontWeight: 700 }}>
        {kind === "command" ? "$" : "✓"}
      </span>
      <span
        style={{
          color: T.text,
          fontSize: kind === "command" ? 22 : 28,
          fontWeight: 500,
          letterSpacing: "0.01em",
          whiteSpace: "nowrap",
        }}
      >
        {detail}
      </span>
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
        padding: isV ? "80px 50px" : "100px 80px",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "space-between",
      }}
    >
      <div
        style={{
          opacity: taglineT,
          transform: `translateY(${(1 - taglineT) * 14}px)`,
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          gap: 20,
        }}
      >
        <div
          style={{
            color: T.muted,
            fontSize: isV ? 18 : 22,
            letterSpacing: "0.32em",
            textTransform: "uppercase",
            fontWeight: 600,
          }}
        >
          Available now
        </div>
        <div
          style={{
            fontSize: isV ? 52 : 80,
            color: T.text,
            textAlign: "center",
            letterSpacing: "-0.02em",
            fontWeight: 700,
            lineHeight: 1.05,
          }}
        >
          System monitor for the <span style={{ color: T.ai }}>AI era</span>.
        </div>
      </div>

      <div
        style={{
          display: "flex",
          flexDirection: "column",
          gap: 20,
          width: isV ? "92%" : 760,
          alignSelf: "center",
        }}
      >
        <InstallRow os="macOS" detail="brew install AsafSaar/dofek/dofek" accent={T.gpu} kind="command" appearAt={6} />
        <InstallRow os="Windows" detail="11 · 10 (19041+)" accent={T.cpu} kind="platform" appearAt={20} />
        <InstallRow os="Linux" detail="Ubuntu · Fedora · Arch" accent={T.mem} kind="platform" appearAt={34} />
      </div>

      <div style={{ opacity: urlT, textAlign: "center", display: "flex", flexDirection: "column", alignItems: "center", gap: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "center", gap: 26 }}>
          <svg width={isV ? 68 : 96} height={isV ? 68 : 96} viewBox="0 0 32 32">
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
          <span
            style={{
              fontSize: isV ? 78 : 120,
              fontWeight: 900,
              color: T.text,
              letterSpacing: "0.02em",
              lineHeight: 1,
            }}
          >
            dofek<span style={{ color: T.cpu }}>.dev</span>
          </span>
        </div>
        <div
          style={{
            color: T.muted,
            fontSize: isV ? 18 : 22,
            letterSpacing: "0.32em",
          }}
        >
          <span style={{ color: T.cpu }}>דופק</span> — pulse.
        </div>
        <div
          style={{
            color: T.muted,
            fontSize: isV ? 14 : 16,
            letterSpacing: "0.4em",
            textTransform: "uppercase",
            marginTop: 6,
            opacity: 0.7,
          }}
        >
          By Asaf Saar
        </div>
      </div>
    </AbsoluteFill>
  );
};
