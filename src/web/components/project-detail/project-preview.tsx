import { PreviewPlaceholder } from "./preview-placeholder";

type PreviewState = "empty" | "loading" | "live";

const PREVIEW_SCALE = 0.6;
const PERCENT = 100;
const SCROLLBAR_WIDTH_PX = 20;
const IFRAME_HEIGHT = `${String(PERCENT / PREVIEW_SCALE)}%`;
const IFRAME_WIDTH = `calc(${String(PERCENT / PREVIEW_SCALE)}% + ${String(SCROLLBAR_WIDTH_PX / PREVIEW_SCALE)}px)`;

interface ProjectPreviewProps {
  readonly state: PreviewState;
  readonly url?: string | null;
}

const ProjectPreview = ({ state, url }: ProjectPreviewProps) => (
  <div className="relative aspect-video w-full overflow-hidden rounded-3xl bg-surface shadow-inset-bevel">
    {state === "live" && url ? (
      <iframe
        src={url}
        title="Project preview"
        className="pointer-events-none absolute top-0 left-0 origin-top-left border-0"
        sandbox="allow-scripts allow-same-origin"
        style={{
          width: IFRAME_WIDTH,
          height: IFRAME_HEIGHT,
          transform: `scale(${String(PREVIEW_SCALE)})`,
        }}
      />
    ) : (
      <div className="absolute inset-0 flex items-center justify-center">
        <PreviewPlaceholder state={state} />
      </div>
    )}
  </div>
);

export type { PreviewState, ProjectPreviewProps };
export { ProjectPreview };
