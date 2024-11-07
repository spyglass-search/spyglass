import classNames from "classnames";
import { ReactNode } from "react";

interface Props {
  label: string;
  children: ReactNode;
  className?: string;
  tabs?: ReactNode;
  icon?: ReactNode;
}

export function Header({ label, children, className, tabs, icon }: Props) {
  return (
    <div
      className={classNames(
        className,
        "p-4",
        "sticky",
        "top-0",
        "bg-neutral-800",
        "flex",
        "flex-row",
        "items-center",
        "z-10",
        "border-b",
        "border-neutral-900",
        "shadow",
      )}
    >
      <div className="flex flex-row items-center gap-4 justify-between w-full min-h-8">
        <div className="font-bold flex flex-row items-center text-lg">
          {icon}
          {label}
        </div>
        <div className="flex flex-row gap-2 place-content-end">{children}</div>
      </div>
      <div className="flex flex-row items-center gap-4">{tabs}</div>
    </div>
  );
}
