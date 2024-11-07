import { ReactNode, useEffect, useState } from "react";
import { invoke } from "../../glue";
import classNames from "classnames";
import { getOperatingSystem, OperatingSystem } from "../../utils";
import launchingGif from "../../assets/wizard/launching-example.gif";

function Key({ keyCode }: { keyCode: string }) {
  const classes = [
    "mx-1",
    "px-1",
    "rounded",
    "border",
    "border-neutral-500",
    "bg-neutral-400",
    "text-black",
  ];

  let label = keyCode;
  if (["Cmd", "CmdOrCtrl"].includes(keyCode)) {
    if (getOperatingSystem() === OperatingSystem.MacOS) {
      label = "⌘";
    } else {
      label = "Ctrl";
    }
  }

  return <div className={classNames(classes)}>{label}</div>;
}

function parseShortcut(shortcut: string): ReactNode {
  const keycodes: string[] = shortcut.split("+");
  return (
    <div className="px-2 flex flex-row">
      {keycodes.map((k) => (
        <Key key={k} keyCode={k} />
      ))}
    </div>
  );
}

export function DisplaySearchbarHelp() {
  const [shortcut, setShortcut] = useState<string>("");
  useEffect(() => {
    const loadShortcut = async () => {
      setShortcut(await invoke<string>("get_shortcut"));
    };

    loadShortcut();
  });

  return (
    <div className="my-auto flex flex-col gap-4 items-center align-middle text-center">
      <div>
        <img
          src={launchingGif}
          alt="Launching in action"
          className="mx-auto rounded-lg w-[196px]"
        />
      </div>
      <div className="text-center text-sm">
        <div className="flex flex-row align-middle items-center text-white place-content-center">
          Use {parseShortcut(shortcut)} to show the searchbar.
        </div>
        <div className="text-xs text-neutral-400">
          You can change the shortcut in your settings.
        </div>
      </div>
      <div className="text-center text-sm">
        <div className="flex flex-row align-middle items-center place-content-center text-white">
          Use {parseShortcut("Esc")} to hide the searchbar.
        </div>
        <div className="text-xs text-neutral-400">
          Clicking elsewhere on your screen will also hide the searchbar.
        </div>
      </div>
    </div>
  );
}
