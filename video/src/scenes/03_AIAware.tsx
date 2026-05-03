import React from "react";
import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { T, Format } from "../ui/Theme";
import { TickerBar } from "../ui/TickerBar";
import { ProcessRow, Category } from "../ui/ProcessRow";

type Row = { name: string; pid: number; cpu: number; mem: number; vram?: number; cat: Category; badge?: "INF" | "IDLE" };

const ROWS: Row[] = [
  { name: "ollama", pid: 4821, cpu: 62.1, mem: 8.4, vram: 14.2, cat: "ai", badge: "INF" },
  { name: "python (torch)", pid: 7733, cpu: 41.7, mem: 6.1, vram: 9.8, cat: "ai", badge: "INF" },
  { name: "code (Cursor)", pid: 1144, cpu: 12.3, mem: 4.2, cat: "dev" },
  { name: "node", pid: 9012, cpu: 7.8, mem: 1.6, cat: "dev" },
  { name: "ComfyUI", pid: 3301, cpu: 22.4, mem: 3.9, vram: 5.2, cat: "ai", badge: "IDLE" },
  { name: "Docker Desktop", pid: 8821, cpu: 3.1, mem: 2.0, cat: "watch" },
  { name: "chrome", pid: 5510, cpu: 4.6, mem: 5.5, cat: "default" },
];

export const AIAware: React.FC<{ format?: Format }> = ({ format = "hero" }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const isV = format === "vertical";

  const headerT = spring({ frame, fps, config: { damping: 20, stiffness: 110 } });
  const aiPulse = 0.5 + 0.5 * Math.sin((frame / fps) * Math.PI * 2);

  return (
    <AbsoluteFill style={{ background: T.bg, fontFamily: T.font }}>
      <TickerBar
        cpu={48}
        gpu={87}
        mem={62}
        aiPill={{ label: "AI", value: "ollama · inferring", pulse: aiPulse > 0.6 }}
      />
      <div style={{ padding: isV ? 40 : 80, paddingTop: 30 }}>
        <div
          style={{
            opacity: headerT,
            transform: `translateY(${(1 - headerT) * 12}px)`,
            fontSize: isV ? 46 : 56,
            fontWeight: 700,
            color: T.text,
            letterSpacing: "-0.01em",
          }}
        >
          <span style={{ color: T.ai }}>AI-aware.</span> Per-process VRAM. Inference state.
        </div>
        <div
          style={{
            marginTop: 36,
            background: T.surf,
            border: `1px solid ${T.bdr2}`,
            borderRadius: 4,
            overflow: "hidden",
          }}
        >
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "20px 1fr 56px 60px 60px 70px 60px",
              padding: "8px 12px",
              borderBottom: `1px solid ${T.bdr2}`,
              fontSize: 9,
              color: T.muted,
              letterSpacing: "0.12em",
              textTransform: "uppercase",
              gap: 8,
              fontWeight: 600,
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
          {ROWS.map((r, i) => {
            const t = spring({ frame: frame - 6 - i * 6, fps, config: { damping: 22, stiffness: 130 } });
            return (
              <div
                key={i}
                style={{
                  opacity: t,
                  transform: `translateX(${(1 - t) * 24}px)`,
                }}
              >
                <ProcessRow {...r} category={r.cat} />
              </div>
            );
          })}
        </div>
      </div>
    </AbsoluteFill>
  );
};
