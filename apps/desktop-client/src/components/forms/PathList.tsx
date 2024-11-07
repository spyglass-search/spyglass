import classNames from "classnames";
import { FormFieldProps } from "./_constants";
import {
  FolderIcon,
  FolderPlusIcon,
  TrashIcon,
} from "@heroicons/react/24/solid";
import { Btn } from "../Btn";
import { invoke } from "../../glue";
import { useState } from "react";

export function PathList({ value, onChange = () => {} }: FormFieldProps) {
  const [paths, setPaths] = useState<string[]>(value as string[]);

  const handleChooseFolder = async () => {
    const folder = await invoke<string>("choose_folder");
    const updatedPaths = [...paths, folder];
    onChange({ oldValue: paths, newValue: updatedPaths });
    setPaths(updatedPaths);
  };
  const handleDelete = (path: string) => {
    const updatedPaths = paths.flatMap((x) => (x === path ? [] : [x]));
    onChange({ oldValue: paths, newValue: updatedPaths });
    setPaths(updatedPaths);
  };

  const handleOpenFolder = async (path: string) => {
    await invoke("open_folder_path", { path });
  };

  return (
    <div className="w-full">
      <div className="border-1 rounded-md bg-stone-700 p-2 h-40 w-full overflow-y-auto">
        {paths.map((path) => (
          <div className="flex items-center p-1.5">
            <button
              className={classNames("flex-none", "mr-2", "group")}
              onClick={() => handleOpenFolder(path)}
            >
              <FolderIcon className={classNames("w-4", "stroke-slate-400")} />
            </button>
            <div className={classNames("grow", "text-sm")}>{path}</div>
            <button
              className={classNames("flex-none", "group")}
              onClick={() => handleDelete(path)}
            >
              <TrashIcon
                className={classNames(
                  "w-4",
                  "fill-slate-400",
                  "group-hover:fill-red-400",
                )}
              />
            </button>
          </div>
        ))}
      </div>
      <Btn onClick={handleChooseFolder} className="ml-auto mt-2">
        <FolderPlusIcon className="mr-2 w-5" />
        Add Folder
      </Btn>
    </div>
  );
}
