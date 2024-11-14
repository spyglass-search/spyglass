import classNames from "classnames";
import { FormFieldProps } from "./_constants";
import { FolderIcon } from "@heroicons/react/16/solid";
import { FolderPlusIcon } from "@heroicons/react/24/solid";
import { Btn } from "../Btn";

export function PathField({ value }: FormFieldProps) {
  const handleOpen = () => {};
  const handleOpenDialog = () => {};

  return (
    <div className="flex flex-row gap-4 w-full">
      <div className="rounded-md bg-stone-700 p-2 grow items-center text-sm join">
        <div
          className={classNames("flex-none", "mr-2", "join-item")}
          onClick={handleOpen}
        >
          <FolderIcon className="w-5 stroke-slate-400" />
        </div>
        <div className="join">{value}</div>
      </div>
      <Btn onClick={handleOpenDialog} className="btn-base">
        <FolderPlusIcon className="mr-2 w-4" />
        Choose Folder
      </Btn>
    </div>
  );
}
