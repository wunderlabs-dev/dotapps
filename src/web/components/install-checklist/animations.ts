const EASE_OUT = "easeOut";

/** Fade + slight scale pop for icons swapping between states. */
const ICON_ENTER = { opacity: 1, scale: 1 };
const ICON_EXIT = { opacity: 0, scale: 0.5 };
const ICON_TRANSITION = { duration: 0.2, ease: EASE_OUT } as const;

/** Fade for label text color transitions. */
const LABEL_ENTER = { opacity: 1 };
const LABEL_EXIT = { opacity: 0 };
const LABEL_TRANSITION = { duration: 0.15, ease: EASE_OUT } as const;

/** Slide-down + fade for each step row entering the checklist. */
const ROW_INITIAL = { opacity: 0, y: -8 };
const ROW_ENTER = { opacity: 1, y: 0 };
const ROW_TRANSITION = { duration: 0.25, ease: EASE_OUT } as const;

export {
  ICON_ENTER,
  ICON_EXIT,
  ICON_TRANSITION,
  LABEL_ENTER,
  LABEL_EXIT,
  LABEL_TRANSITION,
  ROW_ENTER,
  ROW_INITIAL,
  ROW_TRANSITION,
};
