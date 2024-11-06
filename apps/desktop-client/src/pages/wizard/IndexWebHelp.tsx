import { GlobeAltIcon } from "@heroicons/react/24/solid";
import discoverTab from "../../assets/wizard/discover-tab.png";

export function IndexWebHelp() {
  return (
    <div className="p-4 bg-neutral-800 h-screen text-left text-neutral-400 flex flex-col gap-4">
      <h1 className="text-2xl flex flex-row items-center gap-2 text-white">
        <GlobeAltIcon className="w-8" />
        <div>Search web context</div>
      </h1>
      <div className="text-sm">
        Add lenses from the{" "}
        <span className="font-bold text-cyan-500">{"Discover"}</span>
        {" tab to begin searching your favorite web content instantly."}
      </div>
      <div>
        <img
          src={discoverTab}
          className="w-[300px] mx-auto rounded shadow-md shadow-cyan-500/50"
        />
      </div>
    </div>
  );
}
