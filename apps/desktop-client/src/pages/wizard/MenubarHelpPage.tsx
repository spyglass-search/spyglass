import { useState } from "react";
import macosExample from "../../assets/wizard/macos-menubar-example.svg";
import windowsExample from "../../assets/wizard/windows-menubar-example.svg";
import { getOperatingSystem, OperatingSystem } from "../../utils";
import classNames from "classnames";

export function MenubarHelpPage() {
  const [opSys, setOpSys] = useState<OperatingSystem>(getOperatingSystem());
  return (
    <div className="my-auto">
      <div className="join">
        <button
          onClick={() => setOpSys(OperatingSystem.MacOS)}
          className={classNames("btn btn-xs join-item", { "btn-success": opSys === OperatingSystem.MacOS })}
        >
          macOS
        </button>
        <button
          onClick={() => setOpSys(OperatingSystem.Windows)}
          className={classNames("btn btn-xs join-item", { "btn-success": opSys === OperatingSystem.Windows })}
        >
          Windows
        </button>
      </div>
      <img
        src={opSys === OperatingSystem.MacOS ? macosExample : windowsExample}
        alt="Location of the Spyglass menu"
        className="h-[128px] mx-auto my-6"
      />
      <div className="font-bold text-lg text-white">
        {`Spyglass lives in your ${opSys === OperatingSystem.MacOS ? 'menubar' : 'system tray'}.`}
      </div>
      <div className="text-sm text-neutral-400 px-8">
        {
          `${opSys === OperatingSystem.MacOS ? 'Left click' : 'Right click'} on the icon to access your library, discover new lenses, and adjust your settings.`
        }
      </div>
    </div>
  );
}
