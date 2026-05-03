import React from "react";
import { T } from "./Theme";

export const Panel: React.FC<{
  title?: string;
  accent?: string;
  style?: React.CSSProperties;
  children?: React.ReactNode;
}> = ({ title, accent = T.cpu, style, children }) => (
  <div
    style={{
      background: T.surf,
      border: `1px solid ${T.bdr2}`,
      borderRadius: 4,
      display: "flex",
      flexDirection: "column",
      overflow: "hidden",
      ...style,
    }}
  >
    {title && (
      <div
        style={{
          fontSize: 11,
          fontWeight: 700,
          letterSpacing: "0.12em",
          textTransform: "uppercase",
          color: accent,
          padding: "8px 14px",
          borderBottom: `1px solid ${T.bdr}`,
          background: "rgba(255,255,255,0.01)",
        }}
      >
        {title}
      </div>
    )}
    <div style={{ flex: 1, padding: 14 }}>{children}</div>
  </div>
);
