import { SvgIconClose, SvgIconMagnifier } from "@/components/icon";
import { Input } from "@/components/ui";

interface SidebarSearchProps {
  readonly query: string;
  readonly onChange: (q: string) => void;
  readonly onClear: () => void;
}

const SidebarSearch = ({ query, onChange, onClear }: SidebarSearchProps) => {
  return (
    <div className="px-6">
      <div className="relative">
        <span className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2">
          <SvgIconMagnifier size="md" color="muted" />
        </span>
        <Input
          inputSize="lg"
          placeholder="Search projects..."
          value={query}
          onChange={(e) => onChange(e.target.value)}
          className="h-12 rounded-lg border-transparent bg-surface pr-8 pl-9 text-base"
        />
        {query && (
          <button
            type="button"
            onClick={onClear}
            className="absolute top-1/2 right-3 -translate-y-1/2 text-foreground-subtle text-sm leading-none hover:text-foreground"
            aria-label="Clear search"
          >
            <SvgIconClose size="sm" />
          </button>
        )}
      </div>
    </div>
  );
};

export type { SidebarSearchProps };
export { SidebarSearch };
