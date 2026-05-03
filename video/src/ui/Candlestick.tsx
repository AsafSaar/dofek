import React from "react";
import { spring, useCurrentFrame, useVideoConfig } from "remotion";
import { T } from "./Theme";

export type Candle = { lo: number; q1: number; q3: number; hi: number };

const seed = (n: number, mul = 9301, add = 49297, mod = 233280) =>
  ((n * mul + add) % mod) / mod;

export const generateCandles = (count: number, base = 35, vol = 28): Candle[] => {
  const out: Candle[] = [];
  let mid = base;
  for (let i = 0; i < count; i++) {
    const drift = (seed(i + 1) - 0.5) * 6;
    mid = Math.max(8, Math.min(96, mid + drift));
    const spread = vol * (0.5 + seed(i + 13) * 0.9);
    const q1 = Math.max(2, mid - spread * 0.25);
    const q3 = Math.min(98, mid + spread * 0.25);
    const lo = Math.max(0, q1 - spread * 0.6 * (0.5 + seed(i + 41)));
    const hi = Math.min(100, q3 + spread * 0.6 * (0.5 + seed(i + 73)));
    out.push({ lo, q1, q3, hi });
  }
  return out;
};

export const Candlestick: React.FC<{
  candles: Candle[];
  width: number;
  height: number;
  appearStartFrame?: number;
  perCandleFrames?: number;
}> = ({ candles, width, height, appearStartFrame = 0, perCandleFrames = 4 }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const n = candles.length;
  const slot = width / n;
  const bodyW = Math.max(3, slot * 0.55);
  const wickW = Math.max(1, slot * 0.08);

  const yFor = (v: number) => height - (v / 100) * height;

  return (
    <svg width={width} height={height} style={{ display: "block" }}>
      {/* faint grid */}
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
      {candles.map((c, i) => {
        const cx = slot * (i + 0.5);
        const appearAt = appearStartFrame + i * perCandleFrames;
        const t = spring({
          frame: frame - appearAt,
          fps,
          config: { damping: 18, stiffness: 120, mass: 0.6 },
        });
        if (t <= 0) return null;
        const yHi = yFor(c.hi);
        const yQ3 = yFor(c.q3);
        const yQ1 = yFor(c.q1);
        const yLo = yFor(c.lo);
        const bodyTop = yQ3;
        const bodyH = Math.max(2, yQ1 - yQ3);
        const up = c.q3 >= c.q1; // always true; color by drift
        const drift = c.q3 + c.q1 - (c.hi + c.lo);
        const color = drift >= 0 ? T.cpu : T.gpu;
        return (
          <g key={i} opacity={t} transform={`translate(0, ${(1 - t) * 6})`}>
            <rect x={cx - wickW / 2} y={yHi} width={wickW} height={yLo - yHi} fill={color} opacity={0.55} />
            <rect
              x={cx - bodyW / 2}
              y={bodyTop}
              width={bodyW}
              height={bodyH}
              fill={color}
              opacity={0.85}
              rx={1.5}
            />
          </g>
        );
      })}
    </svg>
  );
};
