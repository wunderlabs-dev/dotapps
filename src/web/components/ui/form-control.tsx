/* eslint-disable local/no-multi-comp -- FormControl + InputLabel are a compound component pair */
import { createContext, useContext } from "react";

import { cn } from "@/lib/cn";
import { Typography } from "./typography";

type FormControlVariant = "default" | "error";

interface FormControlContextValue {
  readonly variant: FormControlVariant;
}

const FormControlContext = createContext<FormControlContextValue>({ variant: "default" });

const useFormControlContext = () => useContext(FormControlContext);

interface FormControlProps extends React.HTMLAttributes<HTMLDivElement> {
  readonly variant?: FormControlVariant;
}

const FormControl = ({ variant = "default", className, children, ...props }: FormControlProps) => (
  <FormControlContext.Provider value={{ variant }}>
    <div
      data-slot="form-control"
      data-variant={variant}
      className={cn("flex flex-col gap-2", className)}
      {...props}
    >
      {children}
    </div>
  </FormControlContext.Provider>
);

type InputLabelProps = Omit<React.LabelHTMLAttributes<HTMLLabelElement>, "color">;

const InputLabel = ({ className, children, ...props }: InputLabelProps) => {
  const { variant } = useFormControlContext();

  return (
    <Typography
      as="label"
      variant="caption"
      color={variant === "error" ? "error" : "muted"}
      className={cn("block", className)}
      {...props}
    >
      {children}
    </Typography>
  );
};

export type { FormControlProps, FormControlVariant, InputLabelProps };
export { FormControl, InputLabel, useFormControlContext };
