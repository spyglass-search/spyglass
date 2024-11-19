import classNames from "classnames";
import { getOperatingSystem, OperatingSystem } from "../utils";
import { XMarkIcon } from "@heroicons/react/16/solid";
import { invoke } from "../glue";

interface Props {
  osStyle?: OperatingSystem;
}

export function CustomTitleBar({ osStyle = getOperatingSystem() }: Props) {
  const handleClose = async () => {
    await invoke("escape");
  };

  const renderButton = () => {
    const baseStyles = [
      "flex",
      "flex-row",
      "justify-center",
      "items-center",
      "group-hover:bg-red-500",
    ];

    if (osStyle === OperatingSystem.MacOS) {
      return (
        <div className="ml-[8px] group">
          <button
            className={classNames(
              baseStyles,
              "btn-circle bg-neutral w-[12px] h-[12px]",
            )}
            onClick={handleClose}
          >
            <XMarkIcon className="w-[10px] text-neutral group-hover:text-black" />
          </button>
        </div>
      );
    } else {
      return (
        <div className="group">
          <div
            className={classNames(
              baseStyles,
              "w-[30px] h-[30px] bg-neutral-900",
            )}
            onClick={handleClose}
          >
            <XMarkIcon className="w-[16px] text-neutral-400 group-hover:text-black" />
          </div>
        </div>
      );
    }
  };

  return (
    <div
      data-tauri-drag-region
      className={classNames([
        "titlebar",
        "flex",
        "flex-row",
        "items-center",
        "bg-neutral-900",
        {
          "place-content-end": osStyle !== OperatingSystem.MacOS,
          "place-content-start": osStyle === OperatingSystem.MacOS,
        },
      ])}
    >
      {renderButton()}
    </div>
  );
}
