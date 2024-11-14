import { BookOpenIcon } from "@heroicons/react/16/solid";
import { UserActionDefinition } from "../../bindings/UserActionDefinition";
import { KeyComponent } from "../../components/KeyComponent";
import { DEFAULT_ACTION } from "./constants";

interface ActionListButtonProps {
  isActive: boolean;
  onClick?: () => void;
}
export function ActionListButton({
  isActive,
  onClick = () => {},
}: ActionListButtonProps) {
  const classes = [
    "flex",
    "flex-row",
    "items-center",
    "border-l",
    "text-sm",
    "text-neutral-500",
    "border-neutral-700",
    "px-3",
    "ml-3",
    "h-8",
    isActive ? "bg-stone-800" : "bg-neutral-900",
    "hover:bg-stone-800",
  ];
  return (
    <button className={classes.join(" ")} onClick={onClick}>
      <KeyComponent>ENTER</KeyComponent>
      <span className="ml-1">to open.</span>
    </button>
  );
}

interface ActionListProps {
  actions: UserActionDefinition[];
  selectedActionIdx: number;
  onClick?: (action: UserActionDefinition) => void;
}

export function ActionsList({
  actions,
  selectedActionIdx,
  onClick = () => {},
}: ActionListProps) {
  const classes = [
    "absolute",
    "bottom-8",
    "h-32",
    "max-h-screen",
    "w-1/2",
    "right-0",
    "z-20",
    "flex",
    "flex-col",
    "overflow-hidden",
    "rounded-tl-lg",
    "bg-stone-800",
    "border-t-2",
    "border-l-2",
    "border-neutral-900",
    "p-1",
  ];

  return (
    <div className={classes.join(" ")}>
      <div className="overflow-y-auto">
        <UserActionComponent
          key={`useraction-0`}
          actionId={`useraction-0`}
          action={DEFAULT_ACTION}
          isSelected={selectedActionIdx === 0}
          onClick={() => onClick(DEFAULT_ACTION)}
        />
        {actions.map((action, idx) => (
          <UserActionComponent
            key={`useraction-${idx + 1}`}
            actionId={`useraction-${idx + 1}`}
            action={action}
            isSelected={selectedActionIdx === idx + 1}
            onClick={() => onClick(action)}
          />
        ))}
      </div>
    </div>
  );
}

interface UserActionProps {
  action: UserActionDefinition;
  isSelected: boolean;
  actionId: string;
  onClick?: () => void;
}

function UserActionComponent({
  action,
  isSelected,
  onClick = () => {},
}: UserActionProps) {
  const classes = [
    "flex",
    "flex-col",
    "py-2",
    "text-sm",
    "text-white",
    "cursor-pointer",
    "active:bg-cyan-900",
    "rounded",
    isSelected ? "bg-cyan-900" : "bg-stone-800",
  ];

  return (
    <div className={classes.join(" ")} onClick={onClick}>
      <div className="flex flex-row px-2">
        <BookOpenIcon className="w-6" />
        <span className="grow">{action.label}</span>
      </div>
    </div>
  );
}
