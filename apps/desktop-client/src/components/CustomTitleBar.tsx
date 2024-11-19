import classNames from "classnames";
import { getOperatingSystem, OperatingSystem } from "../utils";
import { MinusIcon, XMarkIcon } from "@heroicons/react/16/solid";
import { invoke } from "../glue";
import macMaximize from "../assets/icons/macos-maximize.svg";

interface Props {
  osStyle?: OperatingSystem;
}

export function CustomTitleBar({ osStyle = getOperatingSystem() }: Props) {
  const handleClose = async () => {
    await invoke("escape");
  };

  const handleMaximize = async () => {
    console.log("to the max!");
  };

  const renderButton = () => {
    const baseStyles = ["flex", "flex-row", "justify-center", "items-center"];

    if (osStyle === OperatingSystem.MacOS) {
      return (
        <div className="ml-[8px] group flex flex-row gap-2">
          <button
            className={classNames(
              baseStyles,
              "btn-circle bg-red-500 w-[12px] h-[12px]",
            )}
            onClick={handleClose}
          >
            <XMarkIcon className="w-[10px] text-transparent group-hover:text-black" />
          </button>
          <button
            className={classNames(
              baseStyles,
              "btn-circle bg-yellow-500 w-[12px] h-[12px]",
            )}
            onClick={handleClose}
          >
            <MinusIcon className="w-[10px] text-transparent group-hover:text-black" />
          </button>
          <button
            className={classNames(
              baseStyles,
              "btn-circle bg-green-500 w-[12px] h-[12px]",
            )}
            onClick={handleMaximize}
          >
            <img
              src={macMaximize}
              className="w-[10px] hidden group-hover:block"
            />
          </button>
        </div>
      );
    } else {
      return (
        <div className="flex flex-row-reverse">
          <div
            className={classNames(
              baseStyles,
              "w-[30px] h-[30px] bg-neutral-900 group hover:bg-red-500",
            )}
            onClick={handleClose}
          >
            <XMarkIcon className="w-[16px] text-neutral-400 group-hover:text-black" />
          </div>
          <div
            className={classNames(
              baseStyles,
              "w-[30px] h-[30px] bg-neutral-900 group hover:bg-neutral-500",
            )}
            onClick={handleMaximize}
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              className="w-[16px] h-[16px] text-neutral-400 group-hover:text-black"
              viewBox="0 0 24 24"
            >
              <path fill="currentColor" d="M4 4h16v16H4zm2 4v10h12V8z" />
            </svg>
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
