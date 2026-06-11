import { useState } from "react";

import { translateError, type UserError } from "@/lib/errors";

interface ErrorState {
  readonly errors: UserError[];
  readonly addError: (error: unknown) => void;
  readonly dismissError: (index: number) => void;
}

const useErrorState = () => {
  const [errors, setErrors] = useState<UserError[]>([]);

  const addError = (error: unknown) => {
    const appError = translateError(error);
    setErrors((prev) => [...prev, appError]);
  };

  const dismissError = (index: number) => {
    setErrors((prev) => prev.filter((_, i) => i !== index));
  };

  return { errors, addError, dismissError };
};

export type { ErrorState };
export { useErrorState };
