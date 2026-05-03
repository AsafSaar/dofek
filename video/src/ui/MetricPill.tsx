import React from "react";
import { T } from "./Theme";

type Variant = "cpu" | "gpu" | "mem" | "ai" | "default";

const colorFor = (v: Variant) => {
  switch (v) {
    case "cpu": return T.cpu;
    case "gpu": return T.gpu;
    case "mem": return T.mem;
    case "ai": return T.ai;
    default: return T.dim;
  }
};

export const MetricPill: React.FC<{
  label: string;
  value: string;
  variant?: Variant;
  pulse?: boolean;
}> = ({ label, value, variant = "default", pulse = false }) => {
  const c = colorFor(variant);
  return (
    <div
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 6,
        padding: "3px 10px",
        border: `1px solid ${variant === "ai" ? "rgba(192,132,252,.3)" : T.bdr2}`,
        borderRadius: 2,
        background: variant === "ai" ? "rgba(192,132,252,.07)" : "rgba(255,255,255,.02)",
        fontFamily: T.font,
        fontSize: 11,
        whiteSpace: "nowrap",
        boxShadow: pulse ? `0 0 12px ${c}55` : "none",
      }}
    >
      <span style={{ color: T.muted, fontSize: 9, letterSpacing: "0.1em", textTransform: "uppercase" }}>{label}</span>
      <span style={{ color: c, fontWeight: 700 }}>{value}</span>
    </div>
  );
};
