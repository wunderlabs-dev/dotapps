import { motion } from "motion/react";

import { Typography } from "@/components/ui";
import { INSTALL_STEPS } from "@/hooks/use-install-flow";
import type { InstallStep, InstallStepStatus } from "@/types";

import { ROW_ENTER, ROW_INITIAL, ROW_TRANSITION } from "./animations";
import { InstallStepRow } from "./install-step-row";

const STEP_LABELS: Record<InstallStep, string> = {
  allocatingResources: "Installing dependencies",
  npmInstalling: "Starting dev server",
  runningServer: "Waiting for server",
};

const STAGGER_DELAY = 0.08;

interface InstallChecklistProps {
  readonly steps: Record<InstallStep, InstallStepStatus>;
}

const InstallChecklist = ({ steps }: InstallChecklistProps) => {
  return (
    <div className="flex flex-col gap-2">
      <Typography variant="h2" className="mb-4">
        Install...
      </Typography>
      {INSTALL_STEPS.map((step, index) => (
        <motion.div
          key={step}
          initial={ROW_INITIAL}
          animate={ROW_ENTER}
          transition={{ ...ROW_TRANSITION, delay: index * STAGGER_DELAY }}
        >
          <InstallStepRow label={STEP_LABELS[step]} status={steps[step]} />
        </motion.div>
      ))}
    </div>
  );
};

export { InstallChecklist };
