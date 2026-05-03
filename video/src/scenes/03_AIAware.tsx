import React from "react";
import { AbsoluteFill, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { T, Format } from "../ui/Theme";
import { TickerBar } from "../ui/TickerBar";

type Cat = "ai" | "dev" | "watch" | "default";
type Row = { name: string; pid: number; cpu: number; mem: number; vram?: number; cat: Cat; badge?: "INF" | "IDLE" };

const ROWS: Row[] = [
  { name: "ollama", pid: 4821, cpu: 62.1, mem: 8.4, vram: 14.2, cat: "ai", badge: "INF" },
  { name: "python (torch)", pid: 7733, cpu: 41.7, mem: 6.1, vram: 9.8, cat: "ai", badge: "INF" },
  { name: "ComfyUI", pid: 3301, cpu: 22.4, mem: 3.9, vram: 5.2, cat: "ai", badge: "IDLE" },
  { name: "stable-diffusion-webui", pid: 3322, cpu: 8.2, mem: 4.8, vram: 6.1, cat: "ai", badge: "IDLE" },
  { name: "code (Cursor)", pid: 1144, cpu: 12.3, mem: 4.2, cat: "dev" },
  { name: "node", pid: 9012, cpu: 7.8, mem: 1.6, cat: "dev" },
  { name: "cargo", pid: 2210, cpu: 18.4, mem: 2.7, cat: "dev" },
  { name: "Docker Desktop", pid: 8821, cpu: 3.1, mem: 2.0, cat: "watch" },
  { name: "postgres", pid: 4410, cpu: 2.4, mem: 1.8, cat: "watch" },
  { name: "chrome", pid: 5510, cpu: 4.6, mem: 5.5, cat: "default" },
];

const accent = (c: Cat) => {
  switch (c) {
    case "ai": return { border: "rgba(192,132,252,.6)", bg: "rgba(192,132,252,.05)", dot: T.ai };
    case "dev": return { border: "rgba(96,165,250,.5)", bg: "rgba(96,165,250,.04)", dot: T.dev };
    case "watch": return { border: "rgba(251,191,36,.5)", bg: "rgba(251,191,36,.04)", dot: T.watch };
    default: return { border: "transparent", bg: "transparent", dot: T.muted };
  }
};

export const AIAware: React.FC<{ format?: Format }> = ({ format = "hero" }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const isV = format === "vertical";

  const headT = spring({ frame, fps, config: { damping: 20, stiffness: 110 } });
  const aiPulse = 0.5 + 0.5 * Math.sin((frame / fps) * Math.PI * 2);

  const ROW_FONT = isV ? 22 : 24;
  const HEAD_FONT = isV ? 13 : 14;

  return (
    <AbsoluteFill style={{ background: T.bg, fontFamily: T.font, display: "flex", flexDirection: "column" }}>
      <TickerBar
        cpu={48}
        gpu={87}
        mem={62}
        aiPill={{ label: "AI", value: "ollama · inferring", pulse: aiPulse > 0.6 }}
      />
      <div
        style={{
          flex: 1,
          padding: isV ? "30px 40px 40px" : "50px 80px 70px",
          display: "flex",
          flexDirection: "column",
          gap: isV ? 24 : 36,
          minHeight: 0,
        }}
      >
        <div
          style={{
            opacity: headT,
            transform: `translateY(${(1 - headT) * 12}px)`,
            display: "flex",
            flexDirection: "column",
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
            Process intelligence
          </div>
          <div
            style={{
              fontSize: isV ? 60 : 84,
              fontWeight: 800,
              color: T.text,
              letterSpacing: "-0.02em",
              lineHeight: 1.05,
            }}
          >
            <span style={{ color: T.ai }}>AI-aware.</span> Per-process VRAM. Inference state.
          </div>
        </div>

        <div
          style={{
            flex: 1,
            background: T.surf,
            border: `1px solid ${T.bdr2}`,
            borderRadius: 6,
            display: "flex",
            flexDirection: "column",
            overflow: "hidden",
            minHeight: 0,
            boxShadow: `0 30px 60px rgba(0,0,0,.45)`,
          }}
        >
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "32px 1fr 110px 110px 110px 130px 110px",
              padding: "16px 22px",
              borderBottom: `1px solid ${T.bdr2}`,
              fontSize: HEAD_FONT,
              color: T.muted,
              letterSpacing: "0.18em",
              textTransform: "uppercase",
              gap: 14,
              fontWeight: 700,
              background: "rgba(255,255,255,0.02)",
            }}
          >
            <span></span>
            <span>Process</span>
            <span style={{ textAlign: "right" }}>PID</span>
            <span style={{ textAlign: "right" }}>CPU</span>
            <span style={{ textAlign: "right" }}>MEM</span>
            <span style={{ textAlign: "right" }}>VRAM</span>
            <span style={{ textAlign: "right" }}>STATE</span>
          </div>
          <div style={{ flex: 1, display: "flex", flexDirection: "column" }}>
            {ROWS.map((r, i) => {
              const t = spring({ frame: frame - 6 - i * 5, fps, config: { damping: 22, stiffness: 130 } });
              const a = accent(r.cat);
              return (
                <div
                  key={i}
                  style={{
                    flex: 1,
                    display: "grid",
                    gridTemplateColumns: "32px 1fr 110px 110px 110px 130px 110px",
                    alignItems: "center",
                    padding: "0 22px",
                    borderLeft: `4px solid ${a.border}`,
                    background: a.bg,
                    color: T.text,
                    gap: 14,
                    borderBottom: i === ROWS.length - 1 ? "none" : `1px solid ${T.bdr}`,
                    fontSize: ROW_FONT,
                    opacity: t,
                    transform: `translateX(${(1 - t) * 24}px)`,
                  }}
                >
                  <span style={{ color: a.dot, fontSize: 16 }}>●</span>
                  <span style={{ fontWeight: 500 }}>{r.name}</span>
                  <span style={{ color: T.muted, textAlign: "right", fontSize: ROW_FONT - 4 }}>{r.pid}</span>
                  <span style={{ color: T.cpu, textAlign: "right", fontWeight: 600 }}>{r.cpu.toFixed(1)}%</span>
                  <span style={{ color: T.mem, textAlign: "right", fontWeight: 600 }}>{r.mem.toFixed(1)}%</span>
                  <span style={{ color: r.vram ? T.gpu : T.muted, textAlign: "right", fontWeight: 600 }}>
                    {r.vram ? `${r.vram.toFixed(1)} GB` : "—"}
                  </span>
                  <span style={{ textAlign: "right" }}>
                    {r.badge && (
                      <span
                        style={{
                          fontSize: ROW_FONT - 8,
                          padding: "4px 12px",
                          borderRadius: 3,
                          fontWeight: 800,
                          letterSpacing: "0.12em",
                          color: r.badge === "INF" ? T.ai : T.muted,
                          background: r.badge === "INF" ? "rgba(192,132,252,.13)" : "rgba(255,255,255,.04)",
                          border: `1px solid ${r.badge === "INF" ? "rgba(192,132,252,.4)" : T.bdr}`,
                        }}
                      >
                        {r.badge}
                      </span>
                    )}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </AbsoluteFill>
  );
};
