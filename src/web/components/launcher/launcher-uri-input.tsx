import type { KeyboardEvent } from "react";

import { SvgIconMagnifier } from "@/components/icon";
import { Input } from "@/components/ui";

interface LauncherUriInputProps {
  readonly value: string;
  readonly submitting: boolean;
  readonly onChange: (value: string) => void;
  readonly onSubmit: () => void;
}

const LauncherUriInput = ({ value, submitting, onChange, onSubmit }: LauncherUriInputProps) => {
  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter") {
      event.preventDefault();
      onSubmit();
    }
  };

  return (
    <div className="border-border-subtle border-b p-3">
      <div className="relative">
        <span className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2">
          <SvgIconMagnifier size="md" color="muted" />
        </span>
        <Input
          name="dotapps-uri"
          inputSize="lg"
          aria-label="App link"
          placeholder="dotapps://app-slug or dotapps://app-slug@1.0.0"
          value={value}
          disabled={submitting}
          onChange={(event) => {
            onChange(event.target.value);
          }}
          onKeyDown={handleKeyDown}
          className="h-11 rounded-lg border-transparent bg-surface pl-9 text-base"
        />
      </div>
    </div>
  );
};

export type { LauncherUriInputProps };
export { LauncherUriInput };
