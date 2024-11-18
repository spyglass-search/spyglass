import { getCurrentWindow } from "@tauri-apps/api/window";
import classNames from "classnames";
import { getOperatingSystem, OperatingSystem } from "../utils";
import { XMarkIcon } from "@heroicons/react/16/solid";

interface Props {
  osStyle?: OperatingSystem;
}

export function CustomTitleBar({ osStyle = getOperatingSystem() }: Props) {
  const appWindow = getCurrentWindow();
  const handleClose = () => {
    if (osStyle === OperatingSystem.MacOS) {
      appWindow.hide();
    } else {
      appWindow.close();
    }
  };

  const renderButton = () => {
    const baseStyles = [
      "flex",
      "flex-row",
      "justify-center",
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
            <XMarkIcon className="w-[10px] ml-[1px] text-neutral group-hover:text-black" />
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
