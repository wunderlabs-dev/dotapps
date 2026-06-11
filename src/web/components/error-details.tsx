import { useState } from "react";

import { Button } from "@/components/ui";

const ErrorDetails = ({
  details,
  onCopy,
}: {
  readonly details: string;
  readonly onCopy: () => void;
}) => {
  const [showDetails, setShowDetails] = useState(false);

  return (
    <div className="mt-2">
      <Button
        variant="link"
        onClick={() => setShowDetails(!showDetails)}
        className="text-foreground-muted text-xs hover:text-foreground"
      >
        {showDetails ? "Hide details" : "Show details"}
      </Button>
      {showDetails && (
        <div className="mt-2 overflow-x-auto whitespace-pre-wrap break-all rounded bg-background p-2 font-mono text-foreground-muted text-xs">
          {details}
          <Button
            variant="link"
            onClick={onCopy}
            className="mt-2 block text-terminal-red hover:text-terminal-red/80"
          >
            Copy to clipboard
          </Button>
        </div>
      )}
    </div>
  );
};

export { ErrorDetails };
