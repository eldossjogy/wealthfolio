import {
  AlertCircle,
  Award,
  Banknote,
  Briefcase,
  Building2,
  Calendar,
  Car,
  Code,
  Coffee,
  CreditCard,
  DollarSign,
  Dumbbell,
  Eye,
  FileText,
  Film,
  Fuel,
  Gift,
  Globe,
  GraduationCap,
  Heart,
  Home,
  Laptop,
  type LucideIcon,
  Palette,
  ParkingCircle,
  Percent,
  PiggyBank,
  Pill,
  Plane,
  Receipt,
  RotateCcw,
  Shield,
  Shirt,
  ShoppingBag,
  ShoppingCart,
  Smartphone,
  Smile,
  Sofa,
  Stethoscope,
  Target,
  Train,
  TrendingUp,
  Truck,
  Tv,
  User,
  UtensilsCrossed,
  Wifi,
  Wine,
  Wrench,
} from "lucide-react";

import { cn } from "@/lib/utils";
import { Icons } from "@wealthfolio/ui";

export type CategoryMeta = {
  name: string;
  color: string | null;
  icon: string | null;
  parentId: string | null;
};

export type CategoryMetaMap = Map<string, CategoryMeta>;

const ICON_REGISTRY: Record<string, LucideIcon> = {
  AlertCircle,
  Award,
  Banknote,
  Briefcase,
  Building: Building2,
  Calendar,
  Car,
  Code,
  Coffee,
  CreditCard,
  DollarSign,
  Dumbbell,
  Eye,
  FileText,
  Film,
  Fuel,
  Gift,
  Globe,
  GraduationCap,
  Heart,
  Home,
  Laptop,
  Palette,
  ParkingCircle,
  Percent,
  PiggyBank,
  Pill,
  Plane,
  Receipt,
  RotateCcw,
  Shield,
  Shirt,
  ShoppingBag,
  ShoppingCart,
  Smartphone,
  Smile,
  Sofa,
  Stethoscope,
  Target,
  Train,
  TrendingUp,
  Truck,
  Tv,
  User,
  UtensilsCrossed,
  Wifi,
  Wine,
  Wrench,
};

export function CategoryIcon({
  icon,
  fallback,
  className,
}: {
  icon: string | null;
  fallback: string;
  className?: string;
}) {
  const LucideIconCmp = icon ? ICON_REGISTRY[icon] : undefined;
  if (LucideIconCmp) return <LucideIconCmp className={cn("h-4 w-4", className)} />;
  return (
    <span className={cn("text-[10px] font-semibold uppercase", className)}>
      {fallback.charAt(0)}
    </span>
  );
}

export function CategoryBadge({
  name,
  color,
  icon,
}: {
  name: string;
  color: string | null;
  icon: string | null;
}) {
  const accent = color ?? "var(--muted-foreground)";
  return (
    <span
      className="inline-flex shrink-0 items-center gap-1 rounded-full px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide"
      style={{
        backgroundColor: color ? `${color}1F` : "var(--muted)",
        color: accent,
      }}
      title={name}
    >
      <CategoryIcon icon={icon} fallback={name} className="h-3 w-3" />
      <span className="max-w-[110px] truncate">{name}</span>
    </span>
  );
}

export function ReviewPill({ label }: { label: string }) {
  return (
    <span
      className="inline-flex shrink-0 items-center gap-1 rounded-full border px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide"
      style={{
        backgroundColor: "hsl(28 65% 55% / 0.10)",
        borderColor: "hsl(28 65% 55% / 0.35)",
        color: "#C28B47",
      }}
    >
      <Icons.AlertCircle className="h-2.5 w-2.5" />
      {label}
    </span>
  );
}
