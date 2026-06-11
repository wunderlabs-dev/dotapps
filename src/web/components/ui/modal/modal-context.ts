import { createContext, useContext } from "react";

interface ModalContextValue {
  readonly headerId: string;
}

const ModalContext = createContext<ModalContextValue | null>(null);

const useModalContext = () => useContext(ModalContext);

export type { ModalContextValue };
export { ModalContext, useModalContext };
