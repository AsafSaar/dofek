import React from "react";
import { AbsoluteFill, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { T, Format } from "../ui/Theme";

type OS = "win" | "linux" | "mac";

const Window: React.FC<{
  os: OS;
  appearAt: number;
  accent: string;
  label: string;
  metric: string;
  metricValue: string;
  bars: number[];
  rows: { k: string; v: string; c?: string }[];
}> = ({ os, appearAt, accent, label, metric, metricValue, bars, rows }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const t = spring({ frame: frame - appearAt, fps, config: { damping: 22, stiffness: 130 } });

  const chrome = (() => {
    if (os === "mac") {
      return (
        <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "14px 18px", borderBottom: `1px solid ${T.bdr2}`, background: T.surf2 }}>
          <span style={{ width: 14, height: 14, borderRadius: 7, background: "#ff5f57" }} />
          <span style={{ width: 14, height: 14, borderRadius: 7, background: "#febc2e" }} />
          <span style={{ width: 14, height: 14, borderRadius: 7, background: "#28c840" }} />
          <span style={{ marginLeft: 14, color: T.dim, fontSize: 14 }}>{label}</span>
        </div>
      );
    }
    if (os === "win") {
      return (
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "12px 18px", borderBottom: `1px solid ${T.bdr2}`, background: T.surf2 }}>
          <span style={{ color: T.dim, fontSize: 14 }}>{label}</span>
          <span style={{ color: T.muted, fontSize: 14, letterSpacing: "0.5em" }}>—  ▢  ✕</span>
        </div>
      );
    }
    return (
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "12px 18px", borderBottom: `1px solid ${T.bdr2}`, background: T.surf2 }}>
        <span style={{ color: T.dim, fontSize: 14 }}>{label}</span>
        <span style={{ color: T.muted, fontSize: 14 }}>_  □  ×</span>
      </div>
    );
  })();

  return (
    <div
      style={{
        flex: 1,
        opacity: t,
        transform: `translateY(${(1 - t) * 30}px) scale(${0.96 + t * 0.04})`,
        background: T.surf,
        border: `1px solid ${T.bdr2}`,
        borderRadius: 8,
        overflow: "hidden",
        boxShadow: `0 24px 50px rgba(0,0,0,.55)`,
        display: "flex",
        flexDirection: "column",
        minHeight: 0,
      }}
    >
      {chrome}
      <div style={{ flex: 1, padding: 22, fontFamily: T.font, color: T.text, display: "flex", flexDirection: "column", gap: 18, minHeight: 0 }}>
        <div style={{ display: "flex", alignItems: "baseline", justifyContent: "space-between" }}>
          <span style={{ color: accent, fontSize: 13, fontWeight: 800, letterSpacing: "0.18em" }}>{metric}</span>
          <span style={{ color: accent, fontSize: 44, fontWeight: 800 }}>{metricValue}</span>
        </div>
        <div style={{ flex: 1, display: "flex", alignItems: "flex-end", gap: 5, minHeight: 140 }}>
          {bars.map((v, i) => (
            <div
              key={i}
              style={{
                flex: 1,
                height: `${v}%`,
                background: `linear-gradient(180deg, ${accent}, ${accent}66)`,
                borderRadius: 2,
                opacity: 0.85,
              }}
            />
          ))}
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 8, borderTop: `1px solid ${T.bdr}`, paddingTop: 14 }}>
          {rows.map((r, i) => (
            <div key={i} style={{ display: "flex", justifyContent: "space-between", fontSize: 15 }}>
              <span style={{ color: T.dim }}>{r.k}</span>
              <span style={{ color: r.c ?? T.text, fontWeight: 600 }}>{r.v}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
};

export const CrossPlatform: React.FC<{ format?: Format }> = ({ format = "hero" }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const isV = format === "vertical";

  const headT = spring({ frame, fps, config: { damping: 20, stiffness: 110 } });

  const winBars = [42, 58, 71, 38, 55, 64, 49, 72, 80, 65, 51, 47, 60, 73];
  const linuxBars = [33, 45, 60, 58, 70, 51, 39, 42, 67, 73, 55, 48, 62, 70];
  const macBars = [55, 62, 49, 71, 64, 58, 73, 81, 76, 60, 52, 47, 68, 75];

  return (
    <AbsoluteFill
      style={{
        background: T.bg,
        fontFamily: T.font,
        padding: isV ? "50px 40px 60px" : "60px 80px 70px",
        display: "flex",
        flexDirection: "column",
        gap: isV ? 30 : 40,
      }}
    >
      <div
        style={{
          opacity: headT,
          transform: `translateY(${(1 - headT) * 14}px)`,
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          gap: 14,
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
          One binary. Three platforms.
        </div>
        <div
          style={{
            fontSize: isV ? 64 : 96,
            fontWeight: 800,
            color: T.text,
            letterSpacing: "-0.02em",
          }}
        >
          <span style={{ color: T.cpu }}>Windows</span>
          <span style={{ color: T.muted, margin: "0 22px" }}>·</span>
          <span style={{ color: T.mem }}>Linux</span>
          <span style={{ color: T.muted, margin: "0 22px" }}>·</span>
          <span style={{ color: T.gpu }}>macOS</span>
        </div>
      </div>
      <div
        style={{
          flex: 1,
          display: "flex",
          flexDirection: isV ? "column" : "row",
          gap: isV ? 22 : 32,
          minHeight: 0,
        }}
      >
        <Window
          os="win"
          appearAt={6}
          accent={T.cpu}
          label="dofek — Windows 11"
          metric="CPU"
          metricValue="68%"
          bars={winBars}
          rows={[
            { k: "i9-13900K", v: "5.4 GHz", c: T.text },
            { k: "Temp", v: "71°C", c: T.warn },
            { k: "Cores", v: "24", c: T.text },
          ]}
        />
        <Window
          os="linux"
          appearAt={20}
          accent={T.mem}
          label="dofek — Ubuntu 24.04"
          metric="MEM"
          metricValue="58%"
          bars={linuxBars}
          rows={[
            { k: "Used", v: "37.1 / 64 GB", c: T.text },
            { k: "Swap", v: "0 / 8 GB", c: T.dim },
            { k: "Buffers", v: "2.4 GB", c: T.dim },
          ]}
        />
        <Window
          os="mac"
          appearAt={34}
          accent={T.gpu}
          label="dofek — macOS Sonoma"
          metric="GPU"
          metricValue="74%"
          bars={macBars}
          rows={[
            { k: "M3 Max", v: "40-core", c: T.text },
            { k: "VRAM", v: "14.2 / 64 GB", c: T.gpu },
            { k: "Power", v: "62 W", c: T.warn },
          ]}
        />
      </div>
    </AbsoluteFill>
  );
};
