import { useState } from "react";

const COPY_FEEDBACK_MS = 2000;

const useCopyFeedback = (text: string) => {
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    (async () => {
      try {
        await navigator.clipboard.writeText(text);
        setCopied(true);
        setTimeout(() => setCopied(false), COPY_FEEDBACK_MS);
      } catch (_: unknown) {
        // Clipboard write failed silently (permission denied or unsupported)
      }
    })();
  };

  return { copied, handleCopy };
};

export { useCopyFeedback };
