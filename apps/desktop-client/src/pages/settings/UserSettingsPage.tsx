import { useEffect, useState } from "react";
import { SettingOpts } from "../../bindings/SettingOpts";
import { invoke } from "../../glue";
import { Btn } from "../../components/Btn";
import { FolderOpenIcon } from "@heroicons/react/20/solid";
import { BtnType, SettingChangeEvents } from "../../components/_constants";
import { FormElement } from "../../components/FormElement";
import classNames from "classnames";

type Setting = [string, SettingOpts];

interface Props {
  errors: { [k: string]: string };
  changes: { [k: string]: string };
}

export function UserSettingsPage() {
  const [userSettings, setUserSettings] = useState<Setting[]>([]);
  const [hasChanges, setHasChanges] = useState<boolean>(false);
  const [restartRequired, setRestartRequired] = useState<boolean>(false);

  useEffect(() => {
    (async () => {
      let settings = await invoke<Setting[]>("load_user_settings");
      console.log(settings);
      setUserSettings(settings);
    })();
  });

  const showSettingsFolder = () => {};
  const handleSave = () => {};
  const handleSettingChange = (
    name: string,
    options: SettingOpts,
    e: SettingChangeEvents,
  ) => {};

  return (
    <div>
      <div className="px-4 pb-2 sticky top-0 bg-neutral-800 py-4 flex flex-row items-center">
        <div className="font-bold">User Settings</div>
        <div className="ml-auto flex flex-row gap-2">
          <Btn onClick={showSettingsFolder} className="btn-sm text-sm">
            <FolderOpenIcon className="mr-1 w-4 h-4" />
            Show Folder
          </Btn>
          <Btn
            onClick={handleSave}
            disabled={hasChanges}
            className="btn-sm text-sm"
            type={hasChanges ? BtnType.Success : BtnType.Default}
          >
            {!hasChanges
              ? "No Changes"
              : restartRequired
                ? "Apply Changes & Restart"
                : "Apply Changes"}
          </Btn>
        </div>
      </div>
      <div className="mt-2 pb-2 flex flex-col gap-4">
        {userSettings.map(([name, options]) => {
          const isVert = ["Path", "PathList", "StringList"].includes(options.form_type);
          return (
            <FormElement
              key={name}
              className={classNames(
                "flex",
                "border-b",
                "border-b-slate-700",
                "pb-4", "px-8",
                isVert
                  ? ["flex-col", "items-start", "gap-2"]
                  : ["justify-between", "flex-row", "gap-8"],
              )}
              onChange={(e) => handleSettingChange(name, options, e)}
              settingName={name}
              settingOptions={options}
            />
          );
        })}
      </div>
    </div>
  );
}
