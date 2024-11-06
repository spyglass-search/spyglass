import { BookmarkIcon } from "@heroicons/react/24/solid";
import chromeIcon from "../../assets/icons/chrome.svg";
import firefoxIcon from "../../assets/icons/firefox.svg";
import { Btn } from "../../components/Btn";

export function IndexBookmarksHelp() {
  return (
    <div className="p-4 bg-neutral-800 h-screen text-left text-neutral-400 flex flex-col gap-4 h-">
      <h1 className="text-2xl flex flex-row items-center gap-2 text-white">
        <BookmarkIcon className="w-8" />
        <div>Search your bookmarks</div>
      </h1>
      <div className="text-sm">
        Easily{" "}
        <span className="font-bold text-cyan-500">
          add URLs to your library
        </span>
        {" & "}
        <span className="font-bold text-cyan-500">sync your bookmarks</span>
        {" with our extensions."}
      </div>
      <Btn
        className="w-full btn-lg"
        href="https://chrome.google.com/webstore/detail/spyglass-search/afhfiojklacoieoanfabefpfngphkmml"
      >
        <img src={chromeIcon} className="w-9 h-9" />
        <div className="ml-2">{"Install for Chrome"}</div>
      </Btn>
      <Btn
        className="w-full btn-lg"
        href="https://addons.mozilla.org/en-US/firefox/addon/spyglass/"
      >
        <img src={firefoxIcon} className="w-9 h-9" />
        <div className="ml-2">{"Install for Firefox"}</div>
      </Btn>
    </div>
  );
}
