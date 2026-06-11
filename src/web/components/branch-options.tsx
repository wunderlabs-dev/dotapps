import { SkeletonBar } from "@/components/ui";

const BranchOptions = ({
  branches,
  currentBranch,
  loading,
  onSelect,
}: {
  readonly branches: readonly string[];
  readonly currentBranch: string;
  readonly loading: boolean;
  readonly onSelect: (branch: string) => void;
}) => {
  if (loading) {
    return (
      <div className="flex flex-col gap-2 p-2">
        <SkeletonBar className="h-8 w-full" />
        <SkeletonBar className="h-8 w-full" />
        <SkeletonBar className="h-8 w-2/3" />
      </div>
    );
  }

  return (
    <>
      {branches.map((branch) => (
        <button
          type="button"
          key={branch}
          onClick={() => onSelect(branch)}
          className={`w-full rounded px-3 py-2 text-left text-foreground ${
            branch === currentBranch
              ? "bg-terminal-green/10 text-terminal-green"
              : "hover:bg-surface-hover"
          }`}
        >
          {branch}
          {branch === currentBranch && " (current)"}
        </button>
      ))}
    </>
  );
};

export { BranchOptions };
