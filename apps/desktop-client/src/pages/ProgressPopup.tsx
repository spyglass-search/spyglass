import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { ModelStatusPayloadWrapper } from "../bindings/ModelStatusPayloadWrapper";

export function ProgressPopup() {
  const [update, setUpdate] = useState<ModelStatusPayloadWrapper | null>(null);

  useEffect(() => {
    const unlisten = listen(
      "progress_update",
      (event: ModelStatusPayloadWrapper) => {
        setUpdate(event);
        console.error("event ", event);
      },
    );

    return () => unlisten.then((fn) => fn());
  }, []);

  return (
    <div className="bg-neutral-800 text-white w-full h-screen">
      <div className="flex flex-col p-4">
        {update ? (
          <>
            <div className="text-sm pb-1">
              {update.payload.msg} - {update.payload.percent}%
            </div>
            <div className="w-full bg-stone-800 h-1 rounded-3xl text-xs">
              <div
                className="bg-cyan-400 h-1 rounded-lg pl-2 flex items-center animate-pulse"
                style={{ width: `${update.payload.percent}%` }}
              ></div>
            </div>
          </>
        ) : (
          <div className="text-sm">Starting download...</div>
        )}
      </div>
    </div>
  );
}
