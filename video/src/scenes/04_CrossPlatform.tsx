import React from "react";
import { AbsoluteFill, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { T, Format } from "../ui/Theme";

const Window: React.FC<{
  os: "win" | "linux" | "mac";
  appearAt: number;
  children: React.ReactNode;
}> = ({ os, appearAt, children }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const t = spring({ frame: frame - appearAt, fps, config: { damping: 22, stiffness: 130 } });

  const chrome = (() => {
    if (os === "mac") {
      return (
        <div style={{ display: "flex", gap: 7, padding: "10px 12px", borderBottom: `1px solid ${T.bdr2}`, background: T.surf2 }}>
          <span style={{ width: 12, height: 12, borderRadius: 6, background: "#ff5f57" }} />
          <span style={{ width: 12, height: 12, borderRadius: 6, background: "#febc2e" }} />
          <span style={{ width: 12, height: 12, borderRadius: 6, background: "#28c840" }} />
          <span style={{ marginLeft: 12, color: T.dim, fontSize: 11 }}>dofek — macOS</span>
        </div>
      );
    }
    if (os === "win") {
      return (
        <div style={{ display: "flex", justifyContent: "space-between", padding: "8px 14px", borderBottom: `1px solid ${T.bdr2}`, background: T.surf2 }}>
          <span style={{ color: T.dim, fontSize: 11 }}>dofek — Windows</span>
          <span style={{ color: T.muted, fontSize: 11, letterSpacing: "0.5em" }}>— ▢ ✕</span>
        </div>
      );
    }
    return (
      <div style={{ display: "flex", justifyContent: "space-between", padding: "8px 14px", borderBottom: `1px solid ${T.bdr2}`, background: T.surf2 }}>
        <span style={{ color: T.dim, fontSize: 11 }}>dofek — Linux</span>
        <span style={{ color: T.muted, fontSize: 11 }}>_  □  ×</span>
      </div>
    );
  })();

  return (
    <div
      style={{
        opacity: t,
        transform: `translateY(${(1 - t) * 24}px) scale(${0.96 + t * 0.04})`,
        background: T.surf,
        border: `1px solid ${T.bdr2}`,
        borderRadius: 6,
        overflow: "hidden",
        boxShadow: `0 18px 40px rgba(0,0,0,.45)`,
        width: "100%",
      }}
    >
      {chrome}
      <div style={{ padding: 16, fontFamily: T.font, color: T.text }}>{children}</div>
    </div>
  );
};

const MiniBars: React.FC<{ color: string; values: number[] }> = ({ color, values }) => (
  <div style={{ display: "flex", alignItems: "flex-end", gap: 3, height: 60 }}>
    {values.map((v, i) => (
      <div key={i} style={{ width: 8, height: `${v}%`, background: color, opacity: 0.55 + (v / 200), borderRadius: 1 }} />
    ))}
  </div>
);

export const CrossPlatform: React.FC<{ format?: Format }> = ({ format = "hero" }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const isV = format === "vertical";

  const headT = spring({ frame, fps, config: { damping: 20, stiffness: 110 } });

  const winBars = [42, 58, 71, 38, 55, 64, 49, 72, 80, 65, 51, 47];
  const linuxBars = [33, 45, 60, 58, 70, 51, 39, 42, 67, 73, 55, 48];
  const macBars = [55, 62, 49, 71, 64, 58, 73, 81, 76, 60, 52, 47];

  return (
    <AbsoluteFill style={{ background: T.bg, fontFamily: T.font, padding: isV ? 40 : 90 }}>
      <div
        style={{
          opacity: headT,
          transform: `translateY(${(1 - headT) * 12}px)`,
          fontSize: isV ? 50 : 64,
          fontWeight: 700,
          color: T.text,
          textAlign: "center",
          marginBottom: 50,
        }}
      >
        <span style={{ color: T.cpu }}>Windows</span>
        <span style={{ color: T.muted, margin: "0 18px" }}>·</span>
        <span style={{ color: T.mem }}>Linux</span>
        <span style={{ color: T.muted, margin: "0 18px" }}>·</span>
        <span style={{ color: T.gpu }}>macOS</span>
      </div>
      <div
        style={{
          display: "flex",
          flexDirection: isV ? "column" : "row",
          gap: 30,
          alignItems: "stretch",
          justifyContent: "center",
        }}
      >
        <div style={{ flex: 1 }}>
          <Window os="win" appearAt={6}>
            <div style={{ color: T.cpu, fontSize: 11, fontWeight: 700, letterSpacing: "0.12em", marginBottom: 10 }}>CPU</div>
            <MiniBars color={T.cpu} values={winBars} />
          </Window>
        </div>
        <div style={{ flex: 1 }}>
          <Window os="linux" appearAt={20}>
            <div style={{ color: T.mem, fontSize: 11, fontWeight: 700, letterSpacing: "0.12em", marginBottom: 10 }}>MEM</div>
            <MiniBars color={T.mem} values={linuxBars} />
          </Window>
        </div>
        <div style={{ flex: 1 }}>
          <Window os="mac" appearAt={34}>
            <div style={{ color: T.gpu, fontSize: 11, fontWeight: 700, letterSpacing: "0.12em", marginBottom: 10 }}>GPU</div>
            <MiniBars color={T.gpu} values={macBars} />
          </Window>
        </div>
      </div>
    </AbsoluteFill>
  );
};
