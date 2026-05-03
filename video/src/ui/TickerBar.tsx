import React from "react";
import { T } from "./Theme";
import { MetricPill } from "./MetricPill";

export const TickerBar: React.FC<{
  hostname?: string;
  clock?: string;
  cpu?: number;
  gpu?: number;
  mem?: number;
  aiPill?: { label: string; value: string; pulse?: boolean };
}> = ({ hostname = "workstation-01", clock = "10:42:08", cpu = 34, gpu = 71, mem = 58, aiPill }) => (
  <div
    style={{
      display: "flex",
      alignItems: "center",
      height: 38,
      background: T.surf,
      borderBottom: `1px solid ${T.bdr2}`,
      padding: "0 16px",
      gap: 10,
      fontFamily: T.font,
    }}
  >
    <div
      style={{
        fontSize: 14,
        fontWeight: 800,
        color: T.cpu,
        letterSpacing: "0.1em",
        paddingRight: 14,
        borderRight: `1px solid ${T.bdr2}`,
        marginRight: 6,
      }}
    >
      dofek
    </div>
    <MetricPill label="CPU" value={`${cpu.toFixed(0)}%`} variant="cpu" />
    <MetricPill label="GPU" value={`${gpu.toFixed(0)}%`} variant="gpu" />
    <MetricPill label="MEM" value={`${mem.toFixed(0)}%`} variant="mem" />
    {aiPill && <MetricPill label={aiPill.label} value={aiPill.value} variant="ai" pulse={aiPill.pulse} />}
    <div style={{ flex: 1 }} />
    <div style={{ color: T.dim, fontSize: 11 }}>{hostname}</div>
    <div
      style={{
        color: T.text,
        fontSize: 11,
        paddingLeft: 14,
        borderLeft: `1px solid ${T.bdr2}`,
      }}
    >
      {clock}
    </div>
  </div>
);
