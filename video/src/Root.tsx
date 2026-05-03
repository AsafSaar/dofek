import React from "react";
import { Composition } from "remotion";
import { loadFont } from "@remotion/google-fonts/JetBrainsMono";
import { HeroSpot60 } from "./compositions/HeroSpot60";
import { VerticalShort30 } from "./compositions/VerticalShort30";

loadFont("normal", {
  weights: ["400", "600", "700", "800"],
  subsets: ["latin"],
});

export const RemotionRoot: React.FC = () => {
  return (
    <>
      <Composition
        id="HeroSpot60"
        component={HeroSpot60}
        durationInFrames={1800}
        fps={30}
        width={1920}
        height={1080}
        defaultProps={{ withAudio: true }}
      />
      <Composition
        id="VerticalShort30"
        component={VerticalShort30}
        durationInFrames={900}
        fps={30}
        width={1080}
        height={1920}
        defaultProps={{ withAudio: true }}
      />
    </>
  );
};
