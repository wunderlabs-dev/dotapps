import { createContext, useContext } from "react";

const cardVariants = ["default", "elevated"] as const;

type CardVariant = (typeof cardVariants)[number];

interface CardContextValue {
  readonly variant: CardVariant;
}

const CardContext = createContext<CardContextValue>({ variant: "default" });

const useCardContext = () => useContext(CardContext);

export type { CardContextValue, CardVariant };
export { CardContext, cardVariants, useCardContext };
