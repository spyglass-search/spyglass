import classNames from "classnames";
import { Btn } from "../components/Btn";
import { useState } from "react";
import { invoke } from "../glue";

// Random gif for your viewing pleasure.
const UPDATE_GIFS: string[] = [
  // Adventure Time
  "10bxTLrpJNS0PC",
  "fm4WhPMzu9hRK",
  "13p77tfexyLtx6",
  "13FBIII8M4IDDi",
  // Futurama
  "gYZ7qO81g4dt6",
];

export function UpdaterPopup() {
  const [isUpdating, setIsUpdating] = useState<boolean>(false);

  const handleUpdate = async () => {
    setIsUpdating(true);
    await invoke("update_and_restart");
  };

  const rando = Math.floor(Math.random() * UPDATE_GIFS.length);
  return (
    <div className="text-white h-screen relative bg-neutral-800">
      <h1 className="text-center text-xl">Update Available!</h1>
      <div className="pt-4 px-8 pb-16 h-64 overflow-hidden text-sm text-center">
        <div className="flex flex-row place-content-center">
          <iframe
            src={`https://giphy.com/embed/${UPDATE_GIFS[rando]}`}
            height="135"
            className="giphy-embed border-none"
          />
        </div>
        <div className="pt-4">{"Thank you for using Spyglass!"}</div>
      </div>
      <div
        className={classNames(
          "fixed",
          "w-full",
          "bottom-0",
          "py-4",
          "px-8",
          "bg-stone-800",
          "z-400",
          "border-t-2",
          "border-stone-900",
        )}
      >
        <div className="flex flex-row place-content-center gap-4">
          <Btn href="https://github.com/spyglass-search/spyglass/releases">
            {"Release Notes"}
          </Btn>
          <Btn onClick={handleUpdate} disabled={isUpdating}>
            {/* <icons::EmojiHappyIcon animate_spin={*is_updating} classes={classNames("mr-2")}/> */}
            Download & Update
          </Btn>
        </div>
      </div>
    </div>
  );
}
