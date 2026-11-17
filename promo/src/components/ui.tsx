import React from "react";
import {
  AbsoluteFill,
  interpolate,
  spring,
  useCurrentFrame,
  useVideoConfig,
} from "remotion";
import { C, MONO, SANS } from "../theme";
import { useLayout } from "../LayoutContext";

/** Spring that starts after `delay` frames. Smooth, no overshoot by default. */
export const useEnter = (delay = 0, damping = 18, mass = 0.8) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  return spring({ frame: frame - delay, fps, config: { damping, mass } });
};

/** Fade + rise style for an element entering at `delay`. */
export const useRise = (delay = 0, distance = 40): React.CSSProperties => {
  const p = useEnter(delay);
  return {
    opacity: interpolate(p, [0, 1], [0, 1]),
    transform: `translateY(${interpolate(p, [0, 1], [distance, 0])}px)`,
  };
};

export const Background: React.FC<{ tone?: "before" | "after"; glow?: string }> = ({
  tone = "after",
  glow,
}) => {
  const frame = useCurrentFrame();
  const drift = frame * 0.4;
  const glowColor =
    glow ?? (tone === "before" ? "rgba(120,120,130,0.10)" : "rgba(36,85,245,0.28)");
  return (
    <AbsoluteFill style={{ background: tone === "before" ? "#0E1015" : C.bg }}>
      <AbsoluteFill
        style={{
          backgroundImage: `linear-gradient(${C.line}22 1px, transparent 1px), linear-gradient(90deg, ${C.line}22 1px, transparent 1px)`,
          backgroundSize: "64px 64px",
          backgroundPosition: `${drift}px ${drift}px`,
          maskImage: "radial-gradient(ellipse at center, black 30%, transparent 75%)",
        }}
      />
      <AbsoluteFill
        style={{
          background: `radial-gradient(ellipse 60% 55% at 50% 45%, ${glowColor}, transparent 70%)`,
        }}
      />
    </AbsoluteFill>
  );
};

const HEAD_PATH =
  "M54 150 C52 100 82 70 120 70 C144 70 160 84 164 100 C178 104 184 118 174 132 L168 126 C164 132 156 138 148 140 C142 160 138 180 138 215 L54 215 Z";
const CHEEK_PATH =
  "M110 117 C120 112 138 112 150 117 C147 134 139 148 128 155 C119 146 113 132 110 117 Z";

/** Falcon mark: a falcon head seen through a magnifier, sparks on what it found.
 *  `color` is the ring + handle; the head is light so it reads on the dark background. */
export const LogoMark: React.FC<{ size: number; color?: string }> = ({
  size,
  color = C.blueLight,
}) => {
  const clipId = React.useId();
  return (
    <svg width={size} height={size} viewBox="0 0 256 256">
      <defs>
        <clipPath id={clipId}>
          <circle cx="116" cy="116" r="80" />
        </clipPath>
      </defs>
      <circle cx="116" cy="116" r="80" fill={C.bg} />
      <g clipPath={`url(#${clipId})`}>
        <path d={HEAD_PATH} fill={C.text} />
        <path d={CHEEK_PATH} fill={C.bg} />
        <circle cx="132" cy="100" r="8" fill={C.bg} />
        <circle cx="134" cy="100" r="4" fill={C.text} />
      </g>
      <circle cx="116" cy="116" r="80" fill="none" stroke={color} strokeWidth="16" />
      <line x1="176" y1="176" x2="234" y2="234" stroke={color} strokeWidth="30" strokeLinecap="round" />
      <g stroke="#F26B2E" strokeWidth="7" strokeLinecap="round">
        <line x1="208" y1="67" x2="220" y2="61" />
        <line x1="219" y1="101" x2="233" y2="100" />
        <line x1="218" y1="138" x2="231" y2="141" />
      </g>
    </svg>
  );
};

export const Wordmark: React.FC<{ size: number; color?: string }> = ({
  size,
  color = C.text,
}) => (
  <div style={{ display: "flex", alignItems: "center", gap: size * 0.3 }}>
    <LogoMark size={size * 1.15} />
    <span
      style={{
        fontFamily: SANS,
        fontSize: size,
        fontWeight: 700,
        letterSpacing: "-0.04em",
        color,
      }}
    >
      falcon
    </span>
  </div>
);

/** Headline revealed word-by-word. Wrap words in *stars* to color them. */
export const Headline: React.FC<{
  text: string;
  size: number;
  delay?: number;
  accent?: string;
  align?: "left" | "center";
  stagger?: number;
  weight?: number;
}> = ({ text, size, delay = 0, accent = C.blueLight, align = "left", stagger = 4, weight = 700 }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const lines = text.split("\n");
  let idx = 0;
  let inHl = false; // *highlight* may span several words
  return (
    <div
      style={{
        fontFamily: SANS,
        fontSize: size,
        fontWeight: weight,
        lineHeight: 1.06,
        letterSpacing: "-0.035em",
        color: C.text,
        textAlign: align,
      }}
    >
      {lines.map((line, li) => (
        <div key={li} style={{ whiteSpace: "nowrap" }}>
          {line.split(" ").map((raw, wi) => {
            const i = idx++;
            const p = spring({ frame: frame - delay - i * stagger, fps, config: { damping: 20 } });
            if (raw.startsWith("*")) inHl = true;
            const hl = inHl;
            if (raw.endsWith("*")) inHl = false;
            const word = raw.replace(/\*/g, "");
            return (
              <span
                key={wi}
                style={{
                  display: "inline-block",
                  marginRight: "0.25em",
                  opacity: p,
                  transform: `translateY(${(1 - p) * 0.45 * size}px)`,
                  filter: `blur(${(1 - p) * 8}px)`,
                  color: hl ? accent : undefined,
                }}
              >
                {word}
              </span>
            );
          })}
        </div>
      ))}
    </div>
  );
};

export const Kicker: React.FC<{ children: React.ReactNode; color?: string; delay?: number }> = ({
  children,
  color = C.blueLight,
  delay = 0,
}) => {
  const { fontScale } = useLayout();
  return (
    <div
      style={{
        ...useRise(delay, 20),
        fontFamily: MONO,
        fontSize: 26 * fontScale,
        fontWeight: 500,
        letterSpacing: "0.14em",
        textTransform: "uppercase",
        color,
      }}
    >
      {children}
    </div>
  );
};

export const Sub: React.FC<{ children: React.ReactNode; delay?: number; size?: number }> = ({
  children,
  delay = 0,
  size = 36,
}) => {
  const { fontScale } = useLayout();
  return (
    <div
      style={{
        ...useRise(delay, 24),
        fontFamily: SANS,
        fontSize: size * fontScale,
        lineHeight: 1.35,
        color: C.muted,
        maxWidth: 760,
      }}
    >
      {children}
    </div>
  );
};

/** macOS-style window: editor, terminal or PR. */
export const Window: React.FC<{
  title: string;
  width: number;
  children: React.ReactNode;
  tilt?: number;
  delay?: number;
  accent?: string;
  style?: React.CSSProperties;
}> = ({ title, width, children, tilt = -8, delay = 0, accent, style }) => {
  const p = useEnter(delay, 22, 1);
  const frame = useCurrentFrame();
  const sway = Math.sin(frame / 45) * 1.2;
  return (
    <div
      style={{
        width,
        borderRadius: 18,
        overflow: "hidden",
        background: "#0A0F1C",
        border: `1px solid ${accent ? accent + "66" : "#ffffff1a"}`,
        boxShadow: `0 40px 90px rgba(0,0,0,0.55), 0 0 80px ${accent ? accent + "22" : "rgba(36,85,245,0.12)"}`,
        opacity: p,
        transform: `perspective(1400px) rotateY(${tilt + sway}deg) rotateX(4deg) translateY(${(1 - p) * 80}px) scale(${0.92 + p * 0.08})`,
        ...style,
      }}
    >
      <div
        style={{
          height: 52,
          background: "#141B2B",
          display: "flex",
          alignItems: "center",
          padding: "0 20px",
          gap: 10,
          borderBottom: "1px solid #ffffff10",
        }}
      >
        {["#FF5F57", "#FEBC2E", "#28C840"].map((c) => (
          <div key={c} style={{ width: 14, height: 14, borderRadius: 7, background: c }} />
        ))}
        <div
          style={{
            flex: 1,
            textAlign: "center",
            fontFamily: MONO,
            fontSize: 20,
            color: C.dim,
            marginRight: 60,
          }}
        >
          {title}
        </div>
      </div>
      <div style={{ padding: "26px 30px" }}>{children}</div>
    </div>
  );
};

/** Code line with simple token coloring. */
export const CodeLine: React.FC<{
  n?: number;
  children: React.ReactNode;
  size?: number;
  bg?: string;
  style?: React.CSSProperties;
}> = ({ n, children, size = 24, bg, style }) => (
  <div
    style={{
      fontFamily: MONO,
      fontSize: size,
      lineHeight: 1.65,
      color: "#D1D5DB",
      whiteSpace: "pre",
      display: "flex",
      background: bg,
      borderRadius: 6,
      ...style,
    }}
  >
    {n !== undefined && (
      <span style={{ color: "#4B5563", width: size * 2.2, textAlign: "right", marginRight: size }}>
        {n}
      </span>
    )}
    <span>{children}</span>
  </div>
);

export const K: React.FC<{ c?: string; children: React.ReactNode }> = ({ c = C.sky, children }) => (
  <span style={{ color: c }}>{children}</span>
);

/** Two-column step layout (stacks in portrait). */
export const SplitLayout: React.FC<{
  left: React.ReactNode;
  right: React.ReactNode;
  leftWidth?: number;
}> = ({ left, right, leftWidth = 680 }) => {
  const { isPortrait, padding } = useLayout();
  return (
    <AbsoluteFill
      style={{
        display: "flex",
        flexDirection: isPortrait ? "column" : "row",
        alignItems: "center",
        justifyContent: "center",
        gap: isPortrait ? 100 : 90,
        padding: isPortrait ? `160px ${padding}px` : `0 ${padding}px`,
      }}
    >
      <div
        style={{
          width: isPortrait ? "100%" : leftWidth,
          display: "flex",
          flexDirection: "column",
          gap: 28,
          flexShrink: 0,
        }}
      >
        {left}
      </div>
      <div style={{ flex: isPortrait ? "none" : 1, display: "flex", justifyContent: "center" }}>
        {right}
      </div>
    </AbsoluteFill>
  );
};

/** Pill badge, e.g. rule id or status. */
export const Pill: React.FC<{
  color: string;
  children: React.ReactNode;
  size?: number;
  style?: React.CSSProperties;
}> = ({ color, children, size = 22, style }) => (
  <div
    style={{
      display: "inline-flex",
      alignItems: "center",
      gap: 10,
      padding: `${size * 0.35}px ${size * 0.8}px`,
      borderRadius: 999,
      background: color + "1F",
      border: `2px solid ${color}88`,
      color,
      fontFamily: MONO,
      fontSize: size,
      fontWeight: 500,
      whiteSpace: "nowrap",
      ...style,
    }}
  >
    {children}
  </div>
);
