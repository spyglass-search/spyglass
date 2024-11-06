import classNames from "classnames";
import { SettingOpts } from "../bindings/SettingOpts";
import { Toggle } from "./forms/Toggle";
import { ReactNode } from "react";

interface Props {
  settingName: string,
  settingOptions: SettingOpts,
  errorMsg?: string;
  onChange?: () => void,
  className?: string,
}

export function FormElement({
  className = "mb-8 flex",
  errorMsg,
  settingName,
  settingOptions,
  onChange = () => {}
}: Props) {

  const parent = settingName.split('.')[0];
  const label = parent !== "_" ? (
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
          {errorMsg ? (<div className="text-red-500 text-xs py-2">{errorMsg}</div>) : null}
      </div>
      {renderElement(settingName, settingOptions, onChange)}
    </div>
  );
}

function renderElement(name: string, opts: SettingOpts, onChange: () => void): ReactNode {
  switch(opts.form_type) {
    case "Bool":
      return (<Toggle name={name} value={JSON.parse(opts.value)} onChange={onChange} />);
    case "PathList":
      return null;
    default:
      return null;
  }
}
