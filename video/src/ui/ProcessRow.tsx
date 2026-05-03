import React from "react";
import { T } from "./Theme";

export type Category = "ai" | "dev" | "watch" | "default";

const accent = (c: Category) => {
  switch (c) {
    case "ai": return { border: "rgba(192,132,252,.6)", bg: "rgba(192,132,252,.05)", dot: T.ai };
    case "dev": return { border: "rgba(96,165,250,.5)", bg: "rgba(96,165,250,.04)", dot: T.dev };
    case "watch": return { border: "rgba(251,191,36,.5)", bg: "rgba(251,191,36,.04)", dot: T.watch };
    default: return { border: "transparent", bg: "transparent", dot: T.muted };
  }
};

export const ProcessRow: React.FC<{
  name: string;
  pid: number;
  cpu: number;
  mem: number;
  vram?: number;
  category?: Category;
  badge?: "INF" | "IDLE" | null;
}> = ({ name, pid, cpu, mem, vram, category = "default", badge = null }) => {
  const a = accent(category);
  return (
    <div
      style={{
        display: "grid",
        gridTemplateColumns: "20px 1fr 56px 60px 60px 70px 60px",
        alignItems: "center",
        padding: "6px 12px",
        borderLeft: `3px solid ${a.border}`,
        background: a.bg,
        fontFamily: T.font,
        fontSize: 12,
        color: T.text,
        gap: 8,
        borderBottom: `1px solid ${T.bdr}`,
      }}
    >
      <span style={{ color: a.dot, fontSize: 10 }}>●</span>
      <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{name}</span>
      <span style={{ color: T.muted, textAlign: "right" }}>{pid}</span>
      <span style={{ color: T.cpu, textAlign: "right" }}>{cpu.toFixed(1)}%</span>
      <span style={{ color: T.mem, textAlign: "right" }}>{mem.toFixed(1)}%</span>
      <span style={{ color: vram ? T.gpu : T.muted, textAlign: "right" }}>
        {vram ? `${vram.toFixed(1)} GB` : "—"}
      </span>
      <span style={{ textAlign: "right" }}>
        {badge && (
          <span
            style={{
              fontSize: 9,
              padding: "1px 6px",
              borderRadius: 2,
              fontWeight: 700,
              letterSpacing: "0.08em",
              color: badge === "INF" ? T.ai : T.muted,
              background: badge === "INF" ? "rgba(192,132,252,.13)" : "rgba(255,255,255,.04)",
              border: `1px solid ${badge === "INF" ? "rgba(192,132,252,.28)" : T.bdr}`,
            }}
          >
            {badge}
          </span>
        )}
      </span>
    </div>
  );
};
