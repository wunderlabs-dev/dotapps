import { translateError } from "@/lib/errors";

/**
 * Human-readable message for failed dotapps commands. The raw invoke() rejection
 * carries the useful detail (registry URL, podman stderr), so prefer it over
 * the generic fallback message.
 */
const describeError = (error: unknown) => {
  const translated = translateError(error);
  return translated.details ?? translated.message;
};

export { describeError };
