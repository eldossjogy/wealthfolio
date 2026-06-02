/**
 * Maps Lucide-named taxonomy icons to app icon components.
 *
 * The DB stores Lucide-style icon names (e.g. `Home`, `ShoppingCart`) on
 * `taxonomy_categories.icon`. This indirection keeps the stored data stable
 * while the UI can choose the concrete icon implementation centrally.
 */
import { Icons, type Icon as PhosphorIcon } from "@wealthfolio/ui";

const ICON_MAP: Record<string, PhosphorIcon> = {
  AlertCircle: Icons.AlertCircle,
  Award: Icons.Award,
  Banknote: Icons.Banknote,
  Briefcase: Icons.Briefcase,
  Building: Icons.Building,
  Calendar: Icons.Calendar,
  Car: Icons.VehicleDuotone,
  Code: Icons.Code,
  Coffee: Icons.Coffee,
  CreditCard: Icons.CreditCard,
  DollarSign: Icons.DollarSign,
  Dumbbell: Icons.Dumbbell,
  Eye: Icons.Eye,
  FileText: Icons.FileText,
  Film: Icons.Film,
  Fuel: Icons.Fuel,
  Gamepad2: Icons.Gamepad2,
  Gift: Icons.Gift,
  Globe: Icons.Globe,
  GraduationCap: Icons.GraduationCap,
  Heart: Icons.Heart,
  Home: Icons.House,
  Laptop: Icons.Laptop,
  Lightbulb: Icons.Lightbulb,
  MoreHorizontal: Icons.MoreHorizontal,
  Palette: Icons.Palette,
  ParkingCircle: Icons.ParkingCircle,
  Percent: Icons.Percent,
  PiggyBank: Icons.PiggyBank,
  Pill: Icons.Pill,
  Plane: Icons.Plane,
  Receipt: Icons.Receipt,
  RotateCcw: Icons.RotateCcw,
  Shield: Icons.Shield,
  Shirt: Icons.Shirt,
  ShoppingBag: Icons.ShoppingBag,
  ShoppingCart: Icons.ShoppingCart,
  Smartphone: Icons.Smartphone,
  Smile: Icons.Smile,
  Sofa: Icons.Sofa,
  Stethoscope: Icons.Stethoscope,
  Tag: Icons.Tag,
  Target: Icons.Target,
  Train: Icons.Train,
  TrendingUp: Icons.TrendingUp,
  Truck: Icons.Truck,
  Tv: Icons.Tv,
  User: Icons.User,
  UtensilsCrossed: Icons.UtensilsCrossed,
  Wifi: Icons.Wifi,
  Wine: Icons.Wine,
  Wrench: Icons.Wrench,
};

/** Resolve a stored Lucide-style icon name to an app icon component (Tag fallback). */
export function resolveCategoryIcon(name: string | null | undefined): PhosphorIcon {
  if (name && ICON_MAP[name]) return ICON_MAP[name];
  return Icons.Tag;
}

/** All known category icon name keys (the same names the seeds use). */
export const CATEGORY_ICON_NAMES = Object.keys(ICON_MAP).sort();
