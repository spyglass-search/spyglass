import { useRef } from "react";
import { FormFieldProps } from "./_constants";
import { KeyComponent } from "../KeyComponent";

export function KeyBinding({ value }: FormFieldProps) {
  const ref = useRef<HTMLInputElement>(null);

  const handleInput = () => {};
  const handleKeydown = () => {};

  const keycodes = (value as string).split("+");
  const keyBinding = keycodes.map((key) => <KeyComponent>{key}</KeyComponent>);
  return (
    <div className="w-full flex flex-row items-center">
      <input
        ref={ref}
        spellCheck="false"
        onInput={handleInput}
        onKeyDown={handleKeydown}
        value={value as string}
        type="text"
        className="grow form-input w-full text-sm rounded bg-stone-700 border-stone-800 mr-4"
      />
      <div className="flex flex-row gap-1 items-center">{keyBinding}</div>
    </div>
  );
}
