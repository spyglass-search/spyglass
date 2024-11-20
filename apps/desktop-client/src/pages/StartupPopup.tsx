import { ArrowPathIcon } from "@heroicons/react/24/solid";
import { useEffect, useRef, useState } from "react";
import { invoke } from "../glue";

export function StartupPopup() {
  const checkInterval = useRef<number | null>(null);
  const [caption, setCaption] = useState<string>("Reticulating splines...");
  const [timeTaken, setTimeTaken] = useState<number>(0);

  const checkStatus = async () => {
    setTimeTaken((t) => t + 1);
    const status = await invoke<string>("get_startup_progress");
    if (status === "DONE" && checkInterval.current) {
      window.clearInterval(checkInterval.current);
    } else {
      setCaption(status);
    }
  };

  useEffect(() => {
    if (!checkInterval.current) {
      checkInterval.current = window.setInterval(checkStatus, 1000);
    }

    return () => {
      if (checkInterval.current) {
        window.clearInterval(checkInterval.current);
      }
    };
  }, []);

  return (
    <div className="bg-neutral-800 py-12 rounded-xl">
      <div className="flex flex-col place-content-center place-items-center">
        <ArrowPathIcon className="animate-spin h-16 w-16" />
        <div className="mt-4 font-medium">{"Starting Spyglass"}</div>
        <div className="mt-1 text-stone-500 text-sm">{caption}</div>
        <div className="mt-1 text-stone-500 text-sm">
          {timeTaken > 0 ? `${timeTaken}s` : null}
        </div>
      </div>
    </div>
  );
}
