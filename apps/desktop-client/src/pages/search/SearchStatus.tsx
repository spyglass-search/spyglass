import { ArrowPathIcon } from "@heroicons/react/24/solid";
import { SearchMeta } from "../../bindings/SearchMeta";
import { KeyComponent } from "../../components/KeyComponent";
import { ArrowDownIcon, ArrowUpIcon } from "@heroicons/react/16/solid";

interface Props {
  meta: SearchMeta | null;
  isThinking: boolean;
}

export function SearchStatus({ meta, isThinking }: Props) {
  if (isThinking) {
    return (
      <>
        <div className="flex flex-row gap-1 items-center">
          <ArrowPathIcon className="w-3 h-3 animate-spin" />
          {"Searching..."}
        </div>
        <div className="ml-auto flex flex-row items-center align-middle pr-2 gap-1">
          <span>{"Use"}</span>
          <KeyComponent>{"/"}</KeyComponent>
          <span>{"to select a lens. Type to search"}</span>
        </div>
      </>
    );
  }

  if (meta) {
    return (
      <div className="flex flex-row justify-between w-full items-center align-middle">
        <div>
          {"Searched "}
          <span className="text-cyan-600">{meta.num_docs}</span>
          {" documents in "}
          <span className="text-cyan-600">
            {meta.wall_time_ms}
            {" ms."}
          </span>
        </div>
        <div className="flex flex-row align-middle items-center gap-1">
          {"Use"}
          <KeyComponent><ArrowUpIcon className="w-2" /></KeyComponent>
          {"and"}
          <KeyComponent><ArrowDownIcon className="w-2" /></KeyComponent>
          {"to select."}
        </div>
      </div>
    );
  } else {
    return null;
  }
}
