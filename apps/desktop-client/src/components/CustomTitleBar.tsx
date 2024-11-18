import { getCurrentWindow } from "@tauri-apps/api/window";
import classNames from "classnames";
import { getOperatingSystem, OperatingSystem } from "../utils";
import { XMarkIcon } from "@heroicons/react/16/solid";

export function CustomTitleBar() {
  const appWindow = getCurrentWindow();
  const os = getOperatingSystem();

  const condClasses = {
    "flex-end": os === OperatingSystem.Windows || os === OperatingSystem.Linux,
    "flex-start": os === OperatingSystem.MacOS
  };

  const handleClose = () => {
    if (os === OperatingSystem.MacOS) {
      appWindow.hide();
    } else {
      appWindow.close();
    }
  };

  return (
    <div data-tauri-drag-region
      className={classNames(["titlebar", "flex", "bg-neutral-900", condClasses])}>
      <div className="ml-[8px] group">
        <button className="btn-circle bg-neutral w-[12px] h-[12px] group-hover:bg-red-500" onClick={handleClose}>
          <XMarkIcon className="w-[10px] ml-[1px] text-neutral group-hover:text-black"/>
        </button>
      </div>
    </div>
  );
}



