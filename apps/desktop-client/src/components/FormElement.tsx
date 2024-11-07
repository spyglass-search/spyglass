import classNames from "classnames";
import { SettingOpts } from "../bindings/SettingOpts";
import { Toggle } from "./forms/Toggle";
import { Text } from "./forms/Text";
import { ReactNode } from "react";
import { SettingChangeEvents } from "./_constants";
import { PathField } from "./forms/PathField";
import { KeyBinding } from "./forms/KeyBinding";
import { StringList } from "./forms/StringList";
import { PathList } from "./forms/PathList";

interface Props {
  settingName: string;
  settingOptions: SettingOpts;
  errorMsg?: string;
  onChange?: (e: SettingChangeEvents) => void;
  className?: string;
}

export function FormElement({
  className = "mb-8 flex",
  errorMsg,
  settingName,
  settingOptions,
  onChange = () => {},
}: Props) {
  const parent = settingName.split(".")[0];
  const label =
    parent !== "_" ? (
      <>
        <span className="text-white">{`${parent}: `}</span>
        <span>{settingOptions.label}</span>
      </>
    ) : (
      <span>{settingOptions.label}</span>
    );

  return (
    <div className={classNames(className)}>
      <div className="mb-2">
        <label className="text-white text-base font-semibold">{label}</label>
        {settingOptions.help_text ? (
          <div className="text-gray-500 text-sm">
            {settingOptions.help_text}
          </div>
        ) : null}
        {errorMsg ? (
          <div className="text-red-500 text-xs py-2">{errorMsg}</div>
        ) : null}
      </div>
      {renderElement(settingName, settingOptions, onChange)}
    </div>
  );
}

function renderElement(
  name: string,
  opts: SettingOpts,
  onChange: (e: SettingChangeEvents) => void,
): ReactNode {
  const value = opts.value;
  switch (opts.form_type) {
    case "Bool":
      return (
        <Toggle
          name={name}
          value={JSON.parse(value)}
          onChange={onChange}
        />
      );
    case "Number":
      return (
        <Text
          name={name}
          value={value}
          onChange={onChange}
          className="text-right w-32"
        />
      );
    case "Text":
      return <Text name={name} value={value} onChange={onChange} />;
    case "Path":
      return <PathField name={name} value={value} onChange={onChange} />;
    case "PathList":
      return <PathList name={name} value={JSON.parse(value)} onChange={onChange} />;
    case "StringList":
      return <StringList name={name} value={JSON.parse(value)} onChange={onChange} />
    case "KeyBinding":
      return <KeyBinding name={name} value={value} onChange={onChange} />;
    default:
      return null;
  }
}
