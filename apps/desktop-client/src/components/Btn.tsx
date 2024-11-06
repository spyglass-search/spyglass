import { MouseEvent, ReactNode } from "react";
import classNames from "classnames";
import { BtnType } from "./_constants";

interface Props {
  type?: BtnType;
  children?: ReactNode;
  className?: string;
  disabled?: boolean;
  onClick?: (event: MouseEvent) => void;
}

export function Btn({
  children,
  className = "",
  disabled = false,
  type = BtnType.Default,
  onClick = () => {},
}: Props) {
  let classes: string[] = [
    "cursor-pointer",
    "flex-row",
    "flex",
    "font-semibold",
    "items-center",
    "leading-5",
    type !== BtnType.Borderless ? "rounded-md" : "",
  ];

  switch (type) {
    case BtnType.Default:
      classes = classes.concat([
        "border-neutral-600",
        "border",
        "hover:bg-neutral-600",
        "active:bg-neutral-700",
        "text-white",
      ]);
      break;
    case BtnType.Borderless:
      classes = classes.concat([
        "hover:bg-neutral-600",
        "active:bg-neutral-700",
        "text-white",
      ]);
      break;
    case BtnType.Danger:
      classes = classes.concat([
        "border",
        "border-red-700",
        "hover:bg-red-700",
        "text-red-500",
        "hover:text-white",
      ]);
      break;
    case BtnType.Success:
      classes = classes.concat(["bg-green-700", "hover:bg-green-900"]);
      break;
    case BtnType.Primary:
      classes = classes.concat(["bg-cyan-600", "hover:bg-cyan-800"]);
      break;
  }

  classes = classes.concat(["text-base", "px-3", "py-2"]);

  return (
    <button
      onClick={onClick}
      className={classNames(classes, className, { "text-stone-400": disabled })}
    >
      <div className={classNames("flex", "flex-row", "gap-1", "items-center")}>
        {children}
      </div>
    </button>
  );
}
