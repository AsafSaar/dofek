import React from "react";
import { AbsoluteFill, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { T, Format } from "../ui/Theme";
import { AreaChart } from "../ui/AreaChart";

const PluginCard: React.FC<{
  title: string;
  accent: string;
  appearAt: number;
  children: React.ReactNode;
}> = ({ title, accent, appearAt, children }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const t = spring({ frame: frame - appearAt, fps, config: { damping: 22, stiffness: 130 } });
  return (
    <div
      style={{
        flex: 1,
        opacity: t,
        transform: `translateX(${(1 - t) * 60}px)`,
        background: T.surf,
        border: `1px solid ${T.bdr2}`,
        borderRadius: 6,
        overflow: "hidden",
        display: "flex",
        flexDirection: "column",
      }}
    >
      <div
        style={{
          padding: "10px 14px",
          borderBottom: `1px solid ${T.bdr}`,
          color: accent,
          fontSize: 12,
          fontWeight: 700,
          letterSpacing: "0.12em",
          background: `${accent}10`,
        }}
      >
        {title}
      </div>
      <div style={{ flex: 1, padding: 16 }}>{children}</div>
    </div>
  );
};

export const Plugins: React.FC<{ format?: Format }> = ({ format = "hero" }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const isV = format === "vertical";

  const headT = spring({ frame, fps, config: { damping: 20, stiffness: 110 } });

  const pingValues = Array.from({ length: 40 }).map((_, i) =>
    Math.max(8, 22 + 14 * Math.sin(i * 0.6) + 6 * Math.cos(i * 1.3))
  );

  return (
    <AbsoluteFill style={{ background: T.bg, fontFamily: T.font, padding: isV ? 36 : 80 }}>
      <div
        style={{
          opacity: headT,
          transform: `translateY(${(1 - headT) * 12}px)`,
          fontSize: isV ? 44 : 54,
          fontWeight: 700,
          color: T.text,
          textAlign: "center",
        }}
      >
        Plugins. <span style={{ color: T.cpu }}>JSON over stdio.</span> Hot-reload.
      </div>
      <div
        style={{
          color: T.dim,
          textAlign: "center",
          fontSize: isV ? 18 : 20,
          marginTop: 12,
          marginBottom: 36,
          letterSpacing: "0.04em",
        }}
      >
        Build your own. Ship in 30 lines.
      </div>
      <div
        style={{
          flex: 1,
          display: "flex",
          flexDirection: isV ? "column" : "row",
          gap: 22,
          minHeight: 0,
        }}
      >
        <PluginCard title="dofek-ollama" accent={T.ai} appearAt={6}>
          <div style={{ color: T.text, fontSize: 13, display: "flex", flexDirection: "column", gap: 8 }}>
            <div style={{ display: "flex", justifyContent: "space-between" }}>
              <span>llama3.1:70b</span>
              <span style={{ color: T.ai, fontWeight: 700 }}>INF</span>
            </div>
            <div style={{ display: "flex", justifyContent: "space-between" }}>
              <span style={{ color: T.dim }}>qwen2.5-coder:32b</span>
              <span style={{ color: T.muted }}>idle</span>
            </div>
            <div style={{ display: "flex", justifyContent: "space-between" }}>
              <span style={{ color: T.dim }}>nomic-embed-text</span>
              <span style={{ color: T.muted }}>idle</span>
            </div>
            <div style={{ marginTop: 10, color: T.muted, fontSize: 11, letterSpacing: "0.1em" }}>VRAM 14.2 / 24 GB</div>
          </div>
        </PluginCard>
        <PluginCard title="dofek-docker" accent={T.dev} appearAt={18}>
          <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
            {["postgres", "redis", "nginx", "api", "worker", "grafana", "prom"].map((c, i) => (
              <span
                key={c}
                style={{
                  padding: "4px 10px",
                  borderRadius: 12,
                  background: i % 3 === 0 ? `${T.mem}22` : `${T.dev}18`,
                  color: i % 3 === 0 ? T.mem : T.dev,
                  border: `1px solid ${i % 3 === 0 ? T.mem : T.dev}55`,
                  fontSize: 11,
                  fontWeight: 600,
                }}
              >
                {c}
              </span>
            ))}
          </div>
          <div style={{ color: T.muted, fontSize: 11, marginTop: 14, letterSpacing: "0.08em" }}>
            7 running · 2 stopped
          </div>
        </PluginCard>
        <PluginCard title="dofek-net-ping" accent={T.cpu} appearAt={30}>
          <div style={{ color: T.text, fontSize: 13, marginBottom: 12 }}>
            <div style={{ display: "flex", justifyContent: "space-between" }}>
              <span>1.1.1.1</span>
              <span style={{ color: T.cpu }}>14 ms</span>
            </div>
            <div style={{ display: "flex", justifyContent: "space-between" }}>
              <span>github.com</span>
              <span style={{ color: T.cpu }}>28 ms</span>
            </div>
          </div>
          <AreaChart
            series={[{ color: T.cpu, values: pingValues, fillOpacity: 0.22 }]}
            width={isV ? 800 : 380}
            height={120}
            drawStartFrame={30}
            drawDurationFrames={60}
          />
        </PluginCard>
      </div>
    </AbsoluteFill>
  );
};
