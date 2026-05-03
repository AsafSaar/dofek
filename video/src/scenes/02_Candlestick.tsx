import React, { useMemo } from "react";
import { AbsoluteFill, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { T, Format } from "../ui/Theme";
import { Candlestick, generateCandles } from "../ui/Candlestick";
import { TickerBar } from "../ui/TickerBar";

export const CandlestickScene: React.FC<{ format?: Format }> = ({ format = "hero" }) => {
  const frame = useCurrentFrame();
  const { fps, width: vw } = useVideoConfig();
  const isV = format === "vertical";

  const candles = useMemo(() => generateCandles(isV ? 32 : 56, 38, 36), [isV]);

  const headerT = spring({ frame, fps, config: { damping: 18, stiffness: 110 } });
  const subT = spring({ frame: frame - 14, fps, config: { damping: 20, stiffness: 110 } });

  const cpuPct = (() => {
    const idx = Math.min(candles.length - 1, Math.floor(((frame - 8) / 4) | 0));
    if (idx < 0) return 0;
    return (candles[idx].q1 + candles[idx].q3) / 2;
  })();

  const chartW = isV ? vw - 100 : vw - 200;
  const chartH = isV ? 760 : 620;

  return (
    <AbsoluteFill style={{ background: T.bg, fontFamily: T.font, display: "flex", flexDirection: "column" }}>
      <TickerBar cpu={cpuPct} gpu={42} mem={61} aiPill={{ label: "AI", value: "ollama · idle", pulse: false }} />
      <div
        style={{
          flex: 1,
          padding: isV ? "30px 40px 40px" : "40px 80px 60px",
          display: "flex",
          flexDirection: "column",
          gap: isV ? 24 : 32,
          minHeight: 0,
        }}
      >
        <div
          style={{
            opacity: headerT,
            transform: `translateY(${(1 - headerT) * 12}px)`,
            display: "flex",
            flexDirection: "column",
            gap: 10,
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
            Candlestick CPU chart
          </div>
          <div
            style={{
              fontSize: isV ? 60 : 88,
              fontWeight: 800,
              color: T.text,
              letterSpacing: "-0.02em",
              lineHeight: 1.05,
            }}
          >
            See <span style={{ color: T.cpu }}>variance</span>, not just averages.
          </div>
          <div
            style={{
              opacity: subT,
              color: T.dim,
              fontSize: isV ? 20 : 24,
              letterSpacing: "0.04em",
            }}
          >
            wick = min/max · body = P25–P75
          </div>
        </div>

        <div
          style={{
            flex: 1,
            background: T.surf,
            border: `1px solid ${T.bdr2}`,
            borderRadius: 6,
            padding: 24,
            display: "flex",
            flexDirection: "column",
            minHeight: 0,
            boxShadow: `0 30px 60px rgba(0,0,0,.45)`,
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 14, marginBottom: 18 }}>
            <span
              style={{
                fontSize: 13,
                color: T.cpu,
                fontWeight: 800,
                letterSpacing: "0.18em",
                textTransform: "uppercase",
                padding: "5px 14px",
                border: `1px solid rgba(56,189,248,.4)`,
                borderRadius: 3,
                background: "rgba(56,189,248,.07)",
              }}
            >
              CPU · CANDLE
            </span>
            <span style={{ color: T.muted, fontSize: 14, letterSpacing: "0.12em" }}>60s WINDOW</span>
            <div style={{ flex: 1 }} />
            <span style={{ color: T.muted, fontSize: 14, letterSpacing: "0.12em" }}>NOW</span>
            <span style={{ color: T.cpu, fontSize: 56, fontWeight: 800 }}>{cpuPct.toFixed(1)}%</span>
          </div>
          <div style={{ flex: 1, minHeight: 0, display: "flex", alignItems: "stretch" }}>
            <div style={{ flex: 1, height: "100%" }}>
              <Candlestick
                candles={candles}
                width={chartW}
                height={chartH}
                appearStartFrame={8}
                perCandleFrames={3}
              />
            </div>
          </div>
        </div>
      </div>
    </AbsoluteFill>
  );
};
