import { ApplicationShell } from "@wealthfolio/ui";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { NavLink, Outlet } from "react-router-dom";

import { cn } from "@/lib/utils";
import { buttonVariants } from "@wealthfolio/ui/components/ui/button-variants";

const sections = [
  {
    href: "/spending",
    title: "Overview",
    icon: <Icons.Dashboard className="size-4" />,
    end: true,
  },
  {
    href: "/spending/transactions",
    title: "Transactions",
    icon: <Icons.Activity className="size-4" />,
  },
  {
    href: "/spending/events",
    title: "Events",
    icon: <Icons.Calendar className="size-4" />,
  },
  {
    href: "/spending/reports",
    title: "Reports",
    icon: <Icons.BarChart className="size-4" />,
  },
];

export default function SpendingLayout() {
  return (
    <ApplicationShell className="px-4 py-6 lg:px-8 lg:py-10">
      <div className="mx-auto flex w-full max-w-6xl flex-col gap-8 lg:flex-row">
        <aside className="lg:w-56 lg:shrink-0">
          <h2 className="text-muted-foreground mb-3 px-2 text-xs font-semibold uppercase tracking-widest">
            Spending
          </h2>
          <nav className="flex flex-col space-y-1">
            {sections.map((s) => (
              <NavLink
                key={s.href}
                to={s.href}
                end={s.end}
                className={({ isActive }) =>
                  cn(
                    buttonVariants({ variant: "ghost" }),
                    "h-9 justify-start rounded-md px-2",
                    isActive ? "bg-muted hover:bg-muted" : "hover:bg-muted/50",
                  )
                }
              >
                <span className="mr-2">{s.icon}</span>
                {s.title}
              </NavLink>
            ))}
          </nav>
        </aside>
        <main className="min-w-0 flex-1">
          <Outlet />
        </main>
      </div>
    </ApplicationShell>
  );
}
