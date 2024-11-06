import { ShareIcon } from "@heroicons/react/24/solid";
import connectionsTab from "../../assets/wizard/connections-tab.png";

export function IndexCloudHelp() {
  return (
    <div className="p-4 bg-neutral-800 h-screen text-left text-neutral-400 flex flex-col gap-4">
        <h1 className="text-2xl flex flex-row items-center gap-2 text-white">
          <ShareIcon className="w-8" />
          <div>Search your cloud accounts</div>
        </h1>
        <div className="text-sm">
            Add accounts in the
            <span className="font-bold text-cyan-500">Connections</span>
            {" tab to search through your Google Drive, Reddit posts, GitHub repos, and more!"}
        </div>
        <div>
            <img src={connectionsTab} className="w-[300px] mx-auto rounded shadow-md shadow-cyan-500/50" />
        </div>
    </div>
  );
}
