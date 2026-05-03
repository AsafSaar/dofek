import React from "react";
import { AbsoluteFill, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { T, Format } from "../ui/Theme";
import { AreaChart } from "../ui/AreaChart";

const PluginCard: React.FC<{
  title: string;
  subtitle: string;
  accent: string;
  appearAt: number;
  children: React.ReactNode;
}> = ({ title, subtitle, accent, appearAt, children }) => {
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
        borderRadius: 8,
        overflow: "hidden",
        display: "flex",
        flexDirection: "column",
        minHeight: 0,
        boxShadow: `0 24px 50px rgba(0,0,0,.5)`,
      }}
    >
      <div
        style={{
          padding: "18px 22px",
          borderBottom: `1px solid ${T.bdr}`,
          background: `${accent}10`,
          display: "flex",
          flexDirection: "column",
          gap: 4,
        }}
      >
        <div style={{ color: accent, fontSize: 18, fontWeight: 800, letterSpacing: "0.06em" }}>{title}</div>
        <div style={{ color: T.muted, fontSize: 13, letterSpacing: "0.08em" }}>{subtitle}</div>
      </div>
      <div style={{ flex: 1, padding: 22, display: "flex", flexDirection: "column", minHeight: 0 }}>{children}</div>
    </div>
  );
};

const Row: React.FC<{ k: string; v: string; vc?: string; dim?: boolean }> = ({ k, v, vc, dim }) => (
  <div style={{ display: "flex", justifyContent: "space-between", padding: "10px 0", borderBottom: `1px solid ${T.bdr}`, fontSize: 16 }}>
    <span style={{ color: dim ? T.muted : T.text }}>{k}</span>
    <span style={{ color: vc ?? T.text, fontWeight: 600, letterSpacing: "0.04em" }}>{v}</span>
  </div>
);

export const Plugins: React.FC<{ format?: Format }> = ({ format = "hero" }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const isV = format === "vertical";

  const headT = spring({ frame, fps, config: { damping: 20, stiffness: 110 } });

  const pingValues = Array.from({ length: 60 }).map((_, i) =>
    Math.max(8, 26 + 16 * Math.sin(i * 0.6) + 8 * Math.cos(i * 1.3))
  );

  return (
    <AbsoluteFill
      style={{
        background: T.bg,
        fontFamily: T.font,
        padding: isV ? "50px 40px 60px" : "60px 80px 70px",
        display: "flex",
        flexDirection: "column",
        gap: isV ? 26 : 36,
      }}
    >
      <div
        style={{
          opacity: headT,
          transform: `translateY(${(1 - headT) * 14}px)`,
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          gap: 12,
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
          Extend it. Hot-reload it.
        </div>
        <div
          style={{
            fontSize: isV ? 56 : 84,
            fontWeight: 800,
            color: T.text,
            letterSpacing: "-0.02em",
            textAlign: "center",
            lineHeight: 1.05,
          }}
        >
          Plugins. <span style={{ color: T.cpu }}>JSON over stdio.</span>
        </div>
        <div style={{ color: T.dim, fontSize: isV ? 18 : 22, letterSpacing: "0.04em" }}>
          Build your own. Ship in 30 lines.
        </div>
      </div>
      <div
        style={{
          flex: 1,
          display: "flex",
          flexDirection: isV ? "column" : "row",
          gap: isV ? 22 : 30,
          minHeight: 0,
        }}
      >
        <PluginCard title="dofek-ollama" subtitle="model status · inference tracking" accent={T.ai} appearAt={6}>
          <Row k="llama3.1:70b" v="INF" vc={T.ai} />
          <Row k="qwen2.5-coder:32b" v="idle" vc={T.muted} dim />
          <Row k="nomic-embed-text" v="idle" vc={T.muted} dim />
          <Row k="codellama:13b" v="idle" vc={T.muted} dim />
          <Row k="mistral:7b" v="idle" vc={T.muted} dim />
          <div style={{ marginTop: "auto", paddingTop: 14, display: "flex", justifyContent: "space-between" }}>
            <span style={{ color: T.muted, fontSize: 13, letterSpacing: "0.12em" }}>VRAM USED</span>
            <span style={{ color: T.ai, fontSize: 28, fontWeight: 800 }}>14.2 / 24 GB</span>
          </div>
        </PluginCard>
        <PluginCard title="dofek-docker" subtitle="container monitoring" accent={T.dev} appearAt={18}>
          <div style={{ display: "flex", flexWrap: "wrap", gap: 8, marginBottom: 18 }}>
            {[
              ["postgres", T.mem],
              ["redis", T.mem],
              ["nginx", T.dev],
              ["api", T.dev],
              ["worker", T.dev],
              ["grafana", T.mem],
              ["prom", T.mem],
              ["traefik", T.dev],
              ["minio", T.dev],
            ].map(([c, color]) => (
              <span
                key={c as string}
                style={{
                  padding: "8px 16px",
                  borderRadius: 14,
                  background: `${color}18`,
                  color: color as string,
                  border: `1px solid ${color}55`,
                  fontSize: 14,
                  fontWeight: 600,
                }}
              >
                {c}
              </span>
            ))}
          </div>
          <div style={{ flex: 1 }} />
          <Row k="Running" v="9" vc={T.mem} />
          <Row k="Stopped" v="2" vc={T.muted} dim />
          <Row k="CPU total" v="14.2%" vc={T.cpu} />
          <Row k="MEM total" v="3.8 GB" vc={T.mem} />
        </PluginCard>
        <PluginCard title="dofek-net-ping" subtitle="TCP latency sampler" accent={T.cpu} appearAt={30}>
          <Row k="1.1.1.1" v="14 ms" vc={T.cpu} />
          <Row k="github.com" v="28 ms" vc={T.cpu} />
          <Row k="api.openai.com" v="42 ms" vc={T.warn} />
          <Row k="hub.docker.com" v="19 ms" vc={T.cpu} />
          <div style={{ flex: 1, marginTop: 14, display: "flex", flexDirection: "column" }}>
            <div style={{ color: T.muted, fontSize: 12, letterSpacing: "0.12em", marginBottom: 6 }}>
              60s WINDOW · github.com
            </div>
            <div style={{ flex: 1, minHeight: 0 }}>
              <AreaChart
                series={[{ color: T.cpu, values: pingValues, fillOpacity: 0.22 }]}
                width={isV ? 800 : 460}
                height={isV ? 240 : 200}
                drawStartFrame={30}
                drawDurationFrames={70}
              />
            </div>
          </div>
        </PluginCard>
      </div>
    </AbsoluteFill>
  );
};
