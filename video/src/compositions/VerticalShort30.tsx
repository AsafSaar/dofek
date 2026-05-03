import React from "react";
import { AbsoluteFill, Series, staticFile, Audio } from "remotion";
import { PulseOpen } from "../scenes/00_PulseOpen";
import { CandlestickScene } from "../scenes/02_Candlestick";
import { AIAware } from "../scenes/03_AIAware";
import { Plugins } from "../scenes/06_Plugins";
import { CTA } from "../scenes/08_CTA";

const SHORT_DURATION = 900;

// 30s @ 30fps = 900 frames
// 0–90 PulseOpen, 90–300 Candlestick, 300–510 AIAware, 510–690 Plugins, 690–900 CTA
export const VerticalShort30: React.FC<{ withAudio?: boolean }> = ({ withAudio = true }) => {
  return (
    <AbsoluteFill>
      <Series>
        <Series.Sequence durationInFrames={90}>
          <PulseOpen format="vertical" />
        </Series.Sequence>
        <Series.Sequence durationInFrames={210}>
          <CandlestickScene format="vertical" />
        </Series.Sequence>
        <Series.Sequence durationInFrames={210}>
          <AIAware format="vertical" />
        </Series.Sequence>
        <Series.Sequence durationInFrames={180}>
          <Plugins format="vertical" />
        </Series.Sequence>
        <Series.Sequence durationInFrames={210}>
          <CTA format="vertical" />
        </Series.Sequence>
      </Series>
      {withAudio && (
        <Audio
          src={staticFile("track.mp3")}
          volume={0.7}
          endAt={SHORT_DURATION}
        />
      )}
    </AbsoluteFill>
  );
};
