import { MouseEvent, ReactNode } from "react";
import classNames from "classnames";
import { BtnAlign, BtnType } from "./_constants";

interface Props {
  type?: BtnType;
  align?: BtnAlign;
  children?: ReactNode;
  className?: string;
  disabled?: boolean;
  href?: string;
  onClick?: (event: MouseEvent) => void;
}

export function Btn({
  children,
  href,
  className = "",
  disabled = false,
  type = BtnType.Default,
  align = BtnAlign.Center,
  onClick = () => {},
}: Props) {
  let classes: string[] = [
    "btn",
    "cursor-pointer",
    "flex-row",
    "flex",
    "font-semibold",
    "items-center",
    type !== BtnType.Borderless ? "rounded-md" : "",
  ];

  switch (type) {
    case BtnType.Default:
      classes = classes.concat(["btn-neutral", "text-white"]);
      break;
    case BtnType.Danger:
      classes = classes.concat(["btn-error"]);
      break;
    case BtnType.Success:
      classes = classes.concat(["btn-success"]);
      break;
    case BtnType.Primary:
      classes = classes.concat(["btn-primary"]);
      break;
  }

  classes = classes.concat(["text-base", "px-3", "py-2"]);

  const conditionalClasses = {
    "btn-disabled": disabled,
    "ml-auto": align === BtnAlign.Right,
    "mx-auto": align === BtnAlign.Center,
  };

  const btnStyles = classNames(classes, className, conditionalClasses);
  const labelStyles = classNames("flex", "flex-row", "gap-1", "items-center");
  return href ? (
    <a onClick={onClick} className={btnStyles} href={href} target="blank">
      <div className={labelStyles}>{children}</div>
    </a>
  ) : (
    <button onClick={onClick} className={btnStyles}>
      <div className={labelStyles}>{children}</div>
    </button>
  );
}
