import React from "react";
import { interpolate, useCurrentFrame } from "remotion";
import { T } from "./Theme";

export type Series = { color: string; values: number[]; fillOpacity?: number };

export const AreaChart: React.FC<{
  series: Series[];
  width: number;
  height: number;
  threshold?: number;
  drawStartFrame?: number;
  drawDurationFrames?: number;
}> = ({ series, width, height, threshold, drawStartFrame = 0, drawDurationFrames = 60 }) => {
  const frame = useCurrentFrame();
  const progress = interpolate(
    frame - drawStartFrame,
    [0, drawDurationFrames],
    [0, 1],
    { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
  );

  const yFor = (v: number) => height - (v / 100) * height;

  const buildPath = (vals: number[]) => {
    const visible = Math.max(2, Math.floor(vals.length * progress));
    const step = width / (vals.length - 1);
    let d = `M 0 ${yFor(vals[0])}`;
    for (let i = 1; i < visible; i++) d += ` L ${i * step} ${yFor(vals[i])}`;
    const lastX = (visible - 1) * step;
    const fill = `${d} L ${lastX} ${height} L 0 ${height} Z`;
    return { line: d, fill };
  };

  return (
    <svg width={width} height={height} style={{ display: "block" }}>
      {[0.25, 0.5, 0.75].map((g) => (
        <line
          key={g}
          x1={0}
          x2={width}
          y1={height * g}
          y2={height * g}
          stroke={T.bdr}
          strokeDasharray="3,5"
          strokeWidth={1}
        />
      ))}
      {series.map((s, idx) => {
        const p = buildPath(s.values);
        return (
          <g key={idx}>
            <path d={p.fill} fill={s.color} opacity={s.fillOpacity ?? 0.18} />
            <path d={p.line} fill="none" stroke={s.color} strokeWidth={2} />
          </g>
        );
      })}
      {threshold !== undefined && (
        <line
          x1={0}
          x2={width}
          y1={yFor(threshold)}
          y2={yFor(threshold)}
          stroke={T.warn}
          strokeWidth={1.2}
          strokeDasharray="6,4"
          opacity={0.7}
        />
      )}
    </svg>
  );
};
