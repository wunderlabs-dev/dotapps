/* eslint-disable @typescript-eslint/no-magic-numbers -- animation keyframe data, not business logic */
import { motion } from "motion/react";

const PRIMARY_ANIMATE = {
  x: [0, 50, -30, 20, 0],
  y: [0, -40, 25, -15, 0],
  scale: [1, 1.15, 0.9, 1.08, 1],
  rotate: [0, 12, -10, 5, 0],
};

const PRIMARY_TRANSITION = {
  duration: 10,
  repeat: Infinity,
  ease: "easeInOut",
} as const;

const SECONDARY_ANIMATE = {
  x: [0, -40, 35, -25, 0],
  y: [0, 30, -25, 20, 0],
  scale: [1, 0.9, 1.15, 0.95, 1],
  rotate: [0, -10, 8, -12, 0],
};

const SECONDARY_TRANSITION = {
  duration: 13,
  repeat: Infinity,
  ease: "easeInOut",
} as const;

const LavaBlob = () => {
  return (
    <div
      className="pointer-events-none absolute inset-0"
      aria-hidden="true"
      style={{
        maskImage:
          "radial-gradient(ellipse 80% 70% at 50% 50%, black 5%, rgba(0,0,0,0.7) 20%, rgba(0,0,0,0.4) 40%, rgba(0,0,0,0.15) 55%, rgba(0,0,0,0.05) 65%, transparent 75%)",
      }}
    >
      <motion.div
        className="absolute top-1/3 left-1/2 h-72 w-72 -translate-x-1/2 rounded-full bg-linear-to-b from-orange-400/30 to-transparent blur-3xl"
        animate={PRIMARY_ANIMATE}
        transition={PRIMARY_TRANSITION}
      />
      <motion.div
        className="absolute top-1/3 left-1/2 h-60 w-60 -translate-x-1/2 rounded-full bg-linear-to-b from-orange-200/25 to-transparent blur-3xl"
        animate={SECONDARY_ANIMATE}
        transition={SECONDARY_TRANSITION}
      />
    </div>
  );
};

export { LavaBlob };
