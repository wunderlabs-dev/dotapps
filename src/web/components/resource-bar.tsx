import { StatPill } from "./stat-pill";

const CPU_WARN_THRESHOLD = 75;
const CPU_ERROR_THRESHOLD = 90;
const MB_PER_GB = 1024;
const DISK_UNAVAILABLE = "N/A";

interface ResourceBarProps {
  readonly cpuPercent: number;
  readonly memoryUsedMb: number;
  readonly memoryTotalMb: number;
  readonly diskAvailableGb?: number;
  readonly diskTotalGb?: number;
}

const toGb = (mb: number) => (mb / MB_PER_GB).toFixed(1);

const cpuColorClass = (cpu: number) => {
  if (cpu >= CPU_ERROR_THRESHOLD) return "text-accent-error";
  if (cpu >= CPU_WARN_THRESHOLD) return "text-accent-warning";
  return "text-accent-success";
};

const ResourceBar = ({
  cpuPercent,
  memoryUsedMb,
  diskAvailableGb,
  diskTotalGb,
}: ResourceBarProps) => {
  return (
    <div className="flex items-center gap-2 rounded-lg bg-background p-1 text-xs uppercase shadow-inset-bevel">
      <StatPill label="RAM" value={`${toGb(memoryUsedMb)} GB`} />
      <StatPill
        label="CPU"
        value={`${cpuPercent.toFixed(1)}%`}
        valueClass={cpuColorClass(cpuPercent)}
      />
      <StatPill
        label="DISK"
        value={
          diskTotalGb !== undefined && diskTotalGb > 0
            ? `${diskAvailableGb?.toFixed(1)} / ${diskTotalGb.toFixed(1)} GB`
            : DISK_UNAVAILABLE
        }
      />
    </div>
  );
};

export type { ResourceBarProps };
export { ResourceBar };
