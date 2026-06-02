import { useEffect, useRef, useState, type ComponentProps, type ReactNode } from "react";
import { ResponsiveContainer } from "recharts";

interface RenderableChartContainerProps extends Omit<
  ComponentProps<typeof ResponsiveContainer>,
  "children" | "initialDimension" | "minWidth" | "minHeight"
> {
  children: ReactNode;
  className?: string;
  fallback?: ReactNode;
}

interface ChartDimensions {
  width: number;
  height: number;
}

export function RenderableChartContainer({
  children,
  className,
  fallback = null,
  width = "100%",
  height = "100%",
  ...responsiveContainerProps
}: RenderableChartContainerProps) {
  const ref = useRef<HTMLDivElement | null>(null);
  const [dimensions, setDimensions] = useState<ChartDimensions | null>(null);

  useEffect(() => {
    const element = ref.current;
    if (!element) return;

    const update = () => {
      const rect = element.getBoundingClientRect();
      const width = Math.round(rect.width);
      const height = Math.round(rect.height);
      const nextDimensions = width > 0 && height > 0 ? { width, height } : null;

      setDimensions((current) => {
        if (
          current?.width === nextDimensions?.width &&
          current?.height === nextDimensions?.height
        ) {
          return current;
        }
        return nextDimensions;
      });
    };

    update();

    if (typeof ResizeObserver === "undefined") return;

    const observer = new ResizeObserver(update);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  return (
    <div ref={ref} className={className}>
      {dimensions ? (
        <ResponsiveContainer
          width={width}
          height={height}
          initialDimension={dimensions}
          minWidth={0}
          minHeight={0}
          {...responsiveContainerProps}
        >
          {children}
        </ResponsiveContainer>
      ) : (
        fallback
      )}
    </div>
  );
}
