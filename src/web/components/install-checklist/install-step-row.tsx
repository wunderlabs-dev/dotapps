/* eslint-disable local/no-multi-comp -- StatusIcon is a tightly coupled sub-variant of InstallStepRow */
import { AnimatePresence, motion } from "motion/react";
import { match } from "ts-pattern";

import { SvgIconCheck, SvgIconCircle, SvgIconClose } from "@/components/icon";
import { Spinner } from "@/components/ui";
import type { InstallStepStatus } from "@/types";

import {
  ICON_ENTER,
  ICON_EXIT,
  ICON_TRANSITION,
  LABEL_ENTER,
  LABEL_EXIT,
  LABEL_TRANSITION,
} from "./animations";

interface InstallStepRowProps {
  readonly label: string;
  readonly status: InstallStepStatus;
}

const StatusIcon = ({ status }: { readonly status: InstallStepStatus }) =>
  match(status)
    .with({ kind: "active" }, () => <Spinner size="md" />)
    .with({ kind: "done" }, () => <SvgIconCheck size="md" color="success" />)
    .with({ kind: "failed" }, () => <SvgIconClose size="md" color="error" />)
    .otherwise(() => <SvgIconCircle size="md" color="muted" />);

const labelClass = (status: InstallStepStatus) =>
  match(status)
    .with({ kind: "pending" }, () => "text-base text-foreground-subtle")
    .otherwise(() => "text-base text-white");

const labelText = (status: InstallStepStatus, label: string) =>
  status.kind === "active" ? `${label} ...` : label;

const InstallStepRow = ({ label, status }: InstallStepRowProps) => {
  return (
    <div className="flex items-center gap-3">
      <AnimatePresence mode="wait">
        <motion.div
          key={status.kind}
          initial={ICON_EXIT}
          animate={ICON_ENTER}
          exit={ICON_EXIT}
          transition={ICON_TRANSITION}
        >
          <StatusIcon status={status} />
        </motion.div>
      </AnimatePresence>
      <AnimatePresence mode="wait">
        <motion.span
          key={`${status.kind}-${label}`}
          initial={LABEL_EXIT}
          animate={LABEL_ENTER}
          exit={LABEL_EXIT}
          transition={LABEL_TRANSITION}
          className={labelClass(status)}
        >
          {labelText(status, label)}
        </motion.span>
      </AnimatePresence>
    </div>
  );
};

export { InstallStepRow };
