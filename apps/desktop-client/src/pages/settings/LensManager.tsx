import {
  ArrowDownOnSquareStackIcon,
  ArrowPathIcon,
  BuildingLibraryIcon,
  FolderOpenIcon,
} from "@heroicons/react/24/solid";
import { Btn } from "../../components/Btn";
import { Header } from "./Header";
import { useEffect, useState } from "react";
import { invoke, listen } from "../../glue";
import { LensResult } from "../../bindings/LensResult";
import { LibraryLens } from "../../components/LibraryLens";

export function LensManager() {
  const [inProgress, setInProgress] = useState<boolean>(false);
  const [lenses, setLenses] = useState<LensResult[]>([]);
  const [uninstalling, setUninstalling] = useState<string[]>([]);

  const handleOpenFolder = async () => {
    await invoke("open_lens_folder");
  };

  const handleUpdate = async () => {
    setInProgress(true);
    await invoke("run_lens_updater");
  };

  const handleUninstall = async (name: string) => {
    if (uninstalling.includes(name)) {
      return;
    }

    setUninstalling((list) => [...list, name]);
    await invoke("uninstall_lens", { name });
  };

  useEffect(() => {
    const init = async () => {
      const installed = await invoke<LensResult[]>("list_installed_lenses");
      setLenses(installed.sort((a, b) => a.label.localeCompare(b.label)));

      return await listen("UpdateLensFinished", () => setInProgress(false));
    };

    const unlisten = init();
    return () => {
      (async () => await unlisten.then((fn) => fn()))();
    };
  }, []);

  return (
    <div>
      <Header
        label="My Library"
        icon={<BuildingLibraryIcon className="w-4 mr-2" />}
      >
        <Btn onClick={handleOpenFolder} className="btn-sm text-sm">
          <FolderOpenIcon className="w-3 mr-1" />
          Lens folder
        </Btn>
        <Btn
          onClick={handleUpdate}
          disabled={inProgress}
          className="btn-sm text-sm"
        >
          {inProgress ? (
            <ArrowPathIcon className="w-3 animate-spin mr-1" />
          ) : (
            <ArrowDownOnSquareStackIcon className="w-3 mr-1" />
          )}
          Update
        </Btn>
      </Header>
      <div className="flex flex-col gap-2 p-4">
        {lenses.map((x) => (
          <LibraryLens
            author={x.author}
            categories={x.categories}
            label={x.label}
            name={x.name}
            description={x.description}
            isInstalled={true}
            onUninstall={() => handleUninstall(x.name)}
          />
        ))}
      </div>
    </div>
  );
}
