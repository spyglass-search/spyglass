import classNames from "classnames";
import { FormFieldProps } from "./_constants";
import {
  FolderIcon,
  FolderPlusIcon,
  TrashIcon,
} from "@heroicons/react/24/solid";
import { Btn } from "../Btn";

export function PathList({ value }: FormFieldProps) {
  let paths = value as string[];

  const handleChooseFolder = () => {};
  const handleOpenFolder = () => {};
  const handleDelete = () => {};

  return (
    <div>
      <div className="border-1 rounded-md bg-stone-700 p-2 h-40 overflow-y-auto">
        {paths.map((path) => (
          <div className="flex items-center p-1.5">
            <button
              className={classNames("flex-none", "mr-2", "group")}
              onClick={handleOpenFolder}
            >
              <FolderIcon className={classNames("w-5", "stroke-slate-400")} />
            </button>
            <div className={classNames("grow", "text-sm")}>{path}</div>
            <button
              className={classNames("flex-none", "group")}
              onClick={handleDelete}
            >
              <TrashIcon
                className={classNames(
                  "w-5",
                  "stroke-slate-400",
                  "group-hover:stroke-white",
                  "group-hover:fill-red-400",
                )}
              />
            </button>
          </div>
        ))}
      </div>
      <div className="mt-4">
        <Btn onClick={handleChooseFolder}>
          <FolderPlusIcon className="mr-2 w-5" />
          Add Folder
        </Btn>
      </div>
    </div>
  );
}
