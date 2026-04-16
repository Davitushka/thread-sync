import { motion, useReducedMotion } from "framer-motion";
import type { ReactNode } from "react";

type Props = {
  children: ReactNode;
  /** Default matches overview KPI tiles. */
  className?: string;
};

export function MotionStatCard({ children, className = "kpi-card" }: Props) {
  const reduceMotion = useReducedMotion();
  return (
    <motion.div
      className={className}
      whileHover={reduceMotion ? undefined : { y: -2, scale: 1.015 }}
      transition={{ type: "spring", stiffness: 420, damping: 30 }}
    >
      {children}
    </motion.div>
  );
}
