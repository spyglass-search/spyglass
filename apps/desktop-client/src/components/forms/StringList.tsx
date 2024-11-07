import classNames from "classnames";
import { FormFieldProps } from "./_constants";
import { TrashIcon } from "@heroicons/react/24/solid";
import { Btn } from "../Btn";
import { useRef, useState } from "react";
import { PlusIcon } from "@heroicons/react/16/solid";

export function StringList({ value, onChange = () => {} }: FormFieldProps) {
  const [strings, setStrings] = useState<string[]>(value as string[]);
  const ref = useRef<HTMLInputElement>(null);

  const handleAdd = () => {
    if (ref.current) {
      const updatedList = [...strings, ref.current.value];
      onChange({ oldValue: strings, newValue: updatedList });
      setStrings(updatedList);
    }
  };
  const handleDelete = (str: string) => {
    const updatedList = strings.flatMap((x) => (x === str ? [] : [x]));
    onChange({ oldValue: strings, newValue: updatedList });
    setStrings(updatedList);
  };

  return (
    <div>
      <div className="border-1 rounded-md bg-stone-700 p-2 h-40 overflow-y-auto">
        {strings.map((str) => (
          <div className="flex items-center rounded-md p-1.5">
            <div className={classNames("grow", "text-sm")}>{str}</div>
            <button
              className={classNames("flex-none", "group")}
              onClick={() => handleDelete(str)}
            >
              <TrashIcon
                className={classNames(
                  "w-4",
                  "fill-slate-400",
                  "",
                  "group-hover:fill-red-500",
                )}
              />
            </button>
          </div>
        ))}
      </div>
      <div className="mt-2 flex flex-row gap-2">
        <input
          ref={ref}
          type="text"
          className="form-input text-sm rounded bg-stone-700 border-stone-800"
          placeholder="html"
          spellCheck={false}
        />
        <Btn onClick={handleAdd}>
          <PlusIcon className="mr-1 w-4" />
          Add
        </Btn>
      </div>
    </div>
  );
}
