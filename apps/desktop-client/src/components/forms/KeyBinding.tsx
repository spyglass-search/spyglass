import { KeyboardEvent, useRef, useState } from "react";
import { FormFieldProps } from "./_constants";
import { KeyComponent } from "../KeyComponent";
import { getOperatingSystem, OperatingSystem } from "../../utils";

export function KeyBinding({ value, onChange = () => {} }: FormFieldProps) {
  const ref = useRef<HTMLInputElement>(null);
  const [keycodes, setKeyCodes] = useState<string[]>(
    (value as string).split("+"),
  );

  const handleKeydown = (e: KeyboardEvent) => {
    const binding = [];
    if (e.metaKey) {
      binding.push("Cmd");
    }

    if (e.ctrlKey) {
      binding.push("Ctrl");
    }

    if (e.altKey) {
      binding.push("Alt");
    }

    if (e.shiftKey) {
      binding.push("Shift");
    }

    const key = e.key.toUpperCase();
    if (!["CONTROL", "META", "ALT", "SHIFT"].includes(key)) {
      binding.push(key === " " ? "Space" : key);
    }

    onChange({ oldValue: keycodes.join("+"), newValue: binding.join("+") });
    setKeyCodes(binding);
  };

  const keyBinding = keycodes.map((key: string) => {
    if (key === "Ctrl") {
      return <KeyComponent>{"CTRL"}</KeyComponent>;
    }
    if (key === "CmdOrCtrl") {
      return getOperatingSystem() === OperatingSystem.MacOS ? (
        <KeyComponent>{"⌘"}</KeyComponent>
      ) : (
        <KeyComponent>{"CTRL"}</KeyComponent>
      );
    } else {
      return <KeyComponent>{key}</KeyComponent>;
    }
  });
  return (
    <div className="w-full flex flex-col">
      <div className="w-full flex flex-row items-center">
        <input
          ref={ref}
          spellCheck="false"
          onKeyDown={handleKeydown}
          value={keycodes.join("+")}
          type="text"
          className="grow form-input w-full text-sm rounded bg-stone-700 border-stone-800 mr-4"
          readOnly
        />
        <div className="flex flex-row gap-1 items-center">{keyBinding}</div>
      </div>
      <div className="text-xs text-gray-400 p-1">
        Press the shortcut you want while the input box is selected.
      </div>
    </div>
  );
}
