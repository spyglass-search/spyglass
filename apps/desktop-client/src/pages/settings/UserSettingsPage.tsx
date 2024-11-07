import { useEffect, useState } from "react";
import { SettingOpts } from "../../bindings/SettingOpts";
import { invoke } from "../../glue";
import { Btn } from "../../components/Btn";
import { FolderOpenIcon } from "@heroicons/react/20/solid";
import { BtnType, SettingChangeEvents } from "../../components/_constants";
import { FormElement } from "../../components/FormElement";
import classNames from "classnames";
import { ArrowPathIcon } from "@heroicons/react/24/solid";

type Setting = [string, SettingOpts];
type SettingMap = { [k: string]: SettingOpts };

export function UserSettingsPage() {
  const [userSettings, setUserSettings] = useState<SettingMap>({});
  const [hasChanges, setHasChanges] = useState<boolean>(false);
  const [restartRequired, setRestartRequired] = useState<boolean>(false);
  const [changes, setChanges] = useState<{ [k: string]: string }>({});
  const [isSaving, setIsSaving] = useState<boolean>(false);

  useEffect(() => {
    (async () => {
      const settings = await invoke<Setting[]>("load_user_settings");
      const settingMap: SettingMap = {};
      settings.forEach(([name, opt]) => {
        settingMap[name] = opt;
      });
      setUserSettings(settingMap);
    })();
  }, []);

  const showSettingsFolder = async () => {
    await invoke("open_settings_folder");
  };

  const handleSave = async () => {
    setHasChanges(false);
    setIsSaving(true);
    await invoke("save_user_settings", {
      settings: changes,
      restart: restartRequired,
    }).then(() => setIsSaving(false));
  };

  const handleSettingChange = (name: string, e: SettingChangeEvents) => {
    const currentSetting = userSettings[name];
    const newValue = JSON.stringify(e.newValue);
    const updatedChanges = { ...changes };

    if (currentSetting.value === newValue) {
      delete updatedChanges[name];
    } else {
      updatedChanges[name] = newValue;
    }

    const restart = Object.keys(updatedChanges)
      .map((name) => userSettings[name].restart_required)
      .reduce((prev, cur) => prev || cur, false);
    setRestartRequired(restart);
    setChanges(updatedChanges);
    setHasChanges(Object.keys(updatedChanges).length > 0);
  };

  const saveLabel = () => {
    if (isSaving) {
      return (
        <>
          <ArrowPathIcon className="w-4 animate-spin mr-2" />
          Saving...
        </>
      );
    } else if (hasChanges) {
      return restartRequired ? (
        <>Apply Changes & Restart</>
      ) : (
        <>Apply Changes</>
      );
    } else {
      return <>No Changes</>;
    }
  };

  return (
    <div>
      <div className="p-4 sticky top-0 bg-neutral-800 flex flex-row items-center">
        <div className="font-bold">User Settings</div>
        <div className="ml-auto flex flex-row gap-2">
          <Btn onClick={showSettingsFolder} className="btn-sm text-sm">
            <FolderOpenIcon className="mr-1 w-4 h-4" />
            Show Folder
          </Btn>
          <Btn
            onClick={handleSave}
            disabled={!hasChanges || isSaving}
            className="btn-sm text-sm"
            type={hasChanges ? BtnType.Success : BtnType.Default}
          >
            {saveLabel()}
          </Btn>
        </div>
      </div>
      <div className="mt-4 flex flex-col gap-4">
        {Object.entries(userSettings).map(([name, options]) => {
          const isVert = ["Path", "PathList"].includes(options.form_type);
          return (
            <FormElement
              key={name}
              className={classNames(
                "flex",
                "border-b",
                "border-b-slate-700",
                "pb-4",
                "px-8",
                isVert
                  ? ["flex-col", "items-start", "gap-2"]
                  : ["justify-between", "flex-row", "gap-8"],
              )}
              onChange={(e) => handleSettingChange(name, e)}
              settingName={name}
              settingOptions={options}
            />
          );
        })}
      </div>
    </div>
  );
}
