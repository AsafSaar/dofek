import React from "react";
import { AbsoluteFill, Series, staticFile, Audio } from "remotion";
import { PulseOpen } from "../scenes/00_PulseOpen";
import { Problem } from "../scenes/01_Problem";
import { CandlestickScene } from "../scenes/02_Candlestick";
import { AIAware } from "../scenes/03_AIAware";
import { CrossPlatform } from "../scenes/04_CrossPlatform";
import { DualUI } from "../scenes/05_DualUI";
import { Plugins } from "../scenes/06_Plugins";
import { Tray } from "../scenes/07_Tray";
import { CTA } from "../scenes/08_CTA";

const HERO_DURATION = 1800;

// Scene durations sum to 1800 frames @ 30 fps = 60s
// 0:0–90, 1:90–270, 2:270–540, 3:540–780, 4:780–960, 5:960–1140, 6:1140–1410, 7:1410–1560, 8:1560–1800
// Audio: drop your licensed bed at video/public/track.mp3.
// Longer tracks are sliced via endAt; missing file causes a render-time error.
export const HeroSpot60: React.FC<{ withAudio?: boolean }> = ({ withAudio = true }) => {
  return (
    <AbsoluteFill>
      <Series>
        <Series.Sequence durationInFrames={90}>
          <PulseOpen />
        </Series.Sequence>
        <Series.Sequence durationInFrames={180}>
          <Problem />
        </Series.Sequence>
        <Series.Sequence durationInFrames={270}>
          <CandlestickScene />
        </Series.Sequence>
        <Series.Sequence durationInFrames={240}>
          <AIAware />
        </Series.Sequence>
        <Series.Sequence durationInFrames={180}>
          <CrossPlatform />
        </Series.Sequence>
        <Series.Sequence durationInFrames={180}>
          <DualUI />
        </Series.Sequence>
        <Series.Sequence durationInFrames={270}>
          <Plugins />
        </Series.Sequence>
        <Series.Sequence durationInFrames={150}>
          <Tray />
        </Series.Sequence>
        <Series.Sequence durationInFrames={240}>
          <CTA />
        </Series.Sequence>
      </Series>
      {withAudio && (
        <Audio
          src={staticFile("track.mp3")}
          volume={0.7}
          endAt={HERO_DURATION}
        />
      )}
    </AbsoluteFill>
  );
};
