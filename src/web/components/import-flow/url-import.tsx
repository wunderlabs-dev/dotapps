import type { ReactNode } from "react";
import { useState } from "react";

import { SvgIconCheck, SvgIconLink, SvgIconRefresh } from "@/components/icon";
import type { ButtonProps } from "@/components/ui";
import { Button, Input } from "@/components/ui";
import type { ImportStatus } from "./types";

type ButtonVariant = NonNullable<ButtonProps<"button">["variant"]>;

interface UrlImportProps {
  readonly status: ImportStatus;
  readonly onImport: (url: string) => void;
  readonly onReset: () => void;
}

const BUTTON_LABELS: Record<ImportStatus, string> = {
  idle: "Import project",
  loading: "Loading...",
  success: "Imported !",
  error: "Try again !",
};

const BUTTON_VARIANTS: Record<ImportStatus, ButtonVariant> = {
  idle: "secondary",
  loading: "secondary",
  success: "success",
  error: "danger",
};

const BUTTON_ICONS: Record<ImportStatus, ReactNode> = {
  idle: <SvgIconLink size="sm" />,
  loading: <SvgIconLink size="sm" />,
  success: <SvgIconCheck size="sm" />,
  error: <SvgIconRefresh size="sm" />,
};

const UrlImport = ({ status, onImport, onReset }: UrlImportProps) => {
  const [url, setUrl] = useState("");

  const handleImport = () => {
    if (status === "error") {
      onReset();
      return;
    }
    onImport(url);
  };

  const isDisabled = (status === "idle" && url.trim().length === 0) || status === "success";
  const isLoading = status === "loading";

  return (
    <div className="flex items-center gap-6">
      <Input
        className="min-w-0 flex-1 rounded-md"
        inputSize="lg"
        startIcon={<SvgIconLink size="sm" />}
        placeholder="Put the URL of the project to import"
        value={url}
        onChange={(e) => setUrl(e.target.value)}
        disabled={isLoading}
      />
      <Button
        className="flex-shrink-0 whitespace-nowrap disabled:border-transparent disabled:text-foreground-subtle disabled:opacity-100"
        type="button"
        size="lg"
        variant={BUTTON_VARIANTS[status]}
        onClick={handleImport}
        disabled={isDisabled || isLoading}
      >
        {BUTTON_ICONS[status]}
        {BUTTON_LABELS[status]}
      </Button>
    </div>
  );
};

export type { UrlImportProps };
export { UrlImport };
