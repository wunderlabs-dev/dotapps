import { motion } from "motion/react";
import type { ComponentProps } from "react";

import { PATHS } from "@/components/browser-panels-paths";
import { cn } from "@/lib/cn";

type BrowserPanelsProps = ComponentProps<"svg">;

const PANEL_INITIAL = { y: 0 };
const PANEL_HOVER = { y: -70 };
const PANEL_TRANSITION = { type: "spring", stiffness: 200, damping: 20 } as const;

const DOT_CLASSES =
  "[&>ellipse]:fill-foreground-subtle/30 [&>ellipse]:transition-[fill] [&>ellipse]:duration-300 hover:[&>ellipse]:fill-foreground";
const STROKE_CLASSES =
  "[&>path]:stroke-foreground-subtle/20 [&>path]:transition-[stroke] [&>path]:duration-300 hover:[&>path]:stroke-foreground";

const BrowserPanels = ({ className, ...props }: BrowserPanelsProps) => {
  return (
    <svg aria-hidden="true" viewBox="0 70 1123 850" className={cn("w-full", className)} {...props}>
      {PATHS.map((path) => (
        <motion.g
          key={path.dots[0].cx}
          initial={PANEL_INITIAL}
          whileHover={PANEL_HOVER}
          transition={PANEL_TRANSITION}
          className={cn("cursor-pointer", DOT_CLASSES, STROKE_CLASSES)}
        >
          <path className="fill-background" d={path.fill} />
          <path fill="none" d={path.stroke} />
          {path.dots.map((dot) => (
            <ellipse key={dot.cx} cx={dot.cx} cy={dot.cy} rx={dot.rx} ry={dot.ry} />
          ))}
        </motion.g>
      ))}
    </svg>
  );
};

export { BrowserPanels };
