import type { ReactNode } from "react";
import { CARD_GLASS } from "./utils";

/**
 * Section title rendered outside the card (matching the spending dashboard),
 * with optional right-aligned meta, above a glass card holding the content.
 */
export function SectionCard({
  title,
  meta,
  children,
}: {
  title: string;
  meta?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="w-full">
      <div className="flex items-baseline justify-between pb-2">
        <h2 className="text-md font-semibold tracking-tight">{title}</h2>
        {meta != null && meta !== "" && (
          <span className="text-muted-foreground/70 text-[11px] font-medium uppercase tracking-wide">
            {meta}
          </span>
        )}
      </div>
      <div className={CARD_GLASS}>{children}</div>
    </div>
  );
}
