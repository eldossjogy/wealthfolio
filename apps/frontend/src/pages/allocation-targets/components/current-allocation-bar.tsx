import type { CategoryAllocation } from "@/lib/types";

interface CurrentAllocationBarProps {
  categories: CategoryAllocation[];
}

export function CurrentAllocationBar({ categories }: CurrentAllocationBarProps) {
  const top = categories.filter((c) => !c.children?.length || c.percentage > 0).slice(0, 8);

  return (
    <div className="space-y-2">
      <div className="flex h-3 w-full overflow-hidden rounded-full">
        {top.map((c) => (
          <div
            key={c.categoryId}
            style={{ width: `${c.percentage}%`, background: c.color }}
            title={`${c.categoryName} ${c.percentage.toFixed(1)}%`}
          />
        ))}
      </div>
      <div className="flex flex-wrap gap-x-3 gap-y-1">
        {top.map((c) => (
          <div key={c.categoryId} className="flex items-center gap-1 text-[11px]">
            <span
              className="inline-block h-2 w-2 shrink-0 rounded-sm"
              style={{ background: c.color }}
            />
            <span className="text-foreground font-medium">{c.categoryName}</span>
            <span className="text-muted-foreground">{c.percentage.toFixed(1)}%</span>
          </div>
        ))}
      </div>
    </div>
  );
}
