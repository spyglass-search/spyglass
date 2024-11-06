import { useState } from "react";
import { SettingChangeEvent } from "../_constants";

interface Props {
  name: string;
  value: boolean;
  onChange?: (e: SettingChangeEvent<boolean>) => void;
  restartRequired?: boolean;
}

export function Toggle({
  name,
  value,
  onChange = () => {},
  restartRequired = false,
}: Props) {
  const [state, setState] = useState<boolean>(value);
  const id = `toggle_${name}`;

  const handleOnChange = () => {
    onChange({
      settingName: name,
      oldValue: state,
      newValue: !state,
      restartRequired,
    });
    setState(!state);
  };

  return (
    <div className="grow items-center pl-4 justify-end flex">
      <label htmlFor={id} className="items-center cursor-pointer">
        <div className="relative">
          <input
            id={id}
            type="checkbox"
            className="sr-only"
            checked={state}
            onChange={handleOnChange}
          />
          <div className="block bg-stone-700 w-14 h-8 rounded-full"></div>
          <div className="text-black dot absolute left-1 top-1 bg-white w-6 h-6 rounded-full transition text-center">
            {state ? "Y" : "N"}
          </div>
        </div>
      </label>
    </div>
  );
}
