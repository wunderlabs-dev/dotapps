import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

/**
 * Merges Tailwind CSS classes safely, handling conflicts.
 * Uses clsx for conditional classes and tailwind-merge for deduplication.
 */
const cn = (...inputs: ClassValue[]) => {
  return twMerge(clsx(inputs));
};

export { cn };
