import { Spinner } from "@/components/ui";
import type { PreviewState } from "./project-preview";

const PLACEHOLDER_MESSAGE: Record<PreviewState, string> = {
  empty: "Start project to see preview",
  loading: "Loading preview...",
  live: "",
};

const PreviewPlaceholder = ({ state }: { readonly state: PreviewState }) => (
  <div className="flex flex-col items-center gap-3 text-foreground-subtle">
    {state === "loading" && <Spinner size="md" />}
    <span className="text-sm">{PLACEHOLDER_MESSAGE[state]}</span>
  </div>
);

export { PreviewPlaceholder };
