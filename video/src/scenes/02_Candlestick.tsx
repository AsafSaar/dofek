import React, { useMemo } from "react";
import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { T, Format } from "../ui/Theme";
import { Candlestick, generateCandles } from "../ui/Candlestick";
import { TickerBar } from "../ui/TickerBar";

export const CandlestickScene: React.FC<{ format?: Format }> = ({ format = "hero" }) => {
  const frame = useCurrentFrame();
  const { fps, width: vw, height: vh } = useVideoConfig();
  const isV = format === "vertical";

  const candles = useMemo(() => generateCandles(isV ? 28 : 44, 38, 36), [isV]);

  const headerT = spring({ frame, fps, config: { damping: 18, stiffness: 110 } });
  const subT = spring({ frame: frame - 14, fps, config: { damping: 20, stiffness: 110 } });

  const chartW = isV ? vw - 120 : vw - 320;
  const chartH = isV ? 600 : 540;
  const cpuPct = (() => {
    const idx = Math.min(candles.length - 1, Math.floor(((frame - 8) / 4) | 0));
    if (idx < 0) return 0;
    return (candles[idx].q1 + candles[idx].q3) / 2;
  })();

  return (
    <AbsoluteFill style={{ background: T.bg, fontFamily: T.font }}>
      <TickerBar cpu={cpuPct} gpu={42} mem={61} aiPill={{ label: "AI", value: "ollama · idle", pulse: false }} />
      <div style={{ padding: isV ? 40 : 70, paddingTop: 40 }}>
        <div
          style={{
            opacity: headerT,
            transform: `translateY(${(1 - headerT) * 12}px)`,
            fontSize: isV ? 50 : 60,
            fontWeight: 700,
            color: T.text,
            letterSpacing: "-0.01em",
          }}
        >
          See <span style={{ color: T.cpu }}>variance</span>, not just averages.
        </div>
        <div
          style={{
            opacity: subT,
            color: T.dim,
            fontSize: isV ? 20 : 22,
            marginTop: 14,
            letterSpacing: "0.04em",
          }}
        >
          wick = min/max · body = P25–P75
        </div>
        <div
          style={{
            marginTop: 38,
            background: T.surf,
            border: `1px solid ${T.bdr2}`,
            borderRadius: 4,
            padding: 18,
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 12 }}>
            <span
              style={{
                fontSize: 11,
                color: T.cpu,
                fontWeight: 700,
                letterSpacing: "0.12em",
                textTransform: "uppercase",
                padding: "2px 8px",
                border: `1px solid rgba(56,189,248,.3)`,
                borderRadius: 2,
                background: "rgba(56,189,248,.07)",
              }}
            >
              CPU · CANDLE
            </span>
            <span style={{ color: T.muted, fontSize: 11, letterSpacing: "0.08em" }}>60s window</span>
            <div style={{ flex: 1 }} />
            <span style={{ color: T.cpu, fontSize: 28, fontWeight: 700 }}>{cpuPct.toFixed(1)}%</span>
          </div>
          <Candlestick candles={candles} width={chartW} height={chartH} appearStartFrame={8} perCandleFrames={4} />
        </div>
      </div>
    </AbsoluteFill>
  );
};
