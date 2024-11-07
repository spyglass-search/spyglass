import { ChevronLeftIcon, ChevronRightIcon } from "@heroicons/react/24/solid";
import { Btn } from "../../components/Btn";
import { useEffect, useState } from "react";
import { MenubarHelpPage } from "./MenubarHelpPage";
import { IndexFilesHelp } from "./IndexFilesHelp";
import { SettingChangeEvents } from "../../components/_constants";
import { DisplaySearchbarHelp } from "./DisplaySearchbarHelp";
import { IndexCloudHelp } from "./IndexCloudHelp";
import { IndexBookmarksHelp } from "./IndexBookmarksHelp";
import { IndexWebHelp } from "./IndexWebHelp";
import { invoke } from "../../glue";

enum WizardStage {
  MenubarHelp = "menubar",
  DisplaySearchbarHelp = "shortcuts",
  IndexCloud = "index-cloud",
  IndexFiles = "index-files",
  IndexBookmarks = "index-bookmarks",
  IndexWeb = "index-web",
  Done = "done",
}

const ORDER: string[] = Object.values(WizardStage);

function prevStage(curStage: WizardStage): WizardStage {
  const idx = ORDER.findIndex((stage) => stage === curStage);
  if (idx > 0) {
    return ORDER[idx - 1] as WizardStage;
  }

  return WizardStage.MenubarHelp;
}

function nextStage(curStage: WizardStage): WizardStage {
  const idx = ORDER.findIndex((stage) => stage === curStage);
  if (idx < ORDER.length - 1) {
    return ORDER[idx + 1] as WizardStage;
  }

  return WizardStage.Done;
}

export function WizardPage() {
  const [stage, setStage] = useState<WizardStage>(WizardStage.MenubarHelp);

  const handleBack = () => setStage(prevStage(stage));
  const handleNext = () => setStage(nextStage(stage));

  // Keep track of various settings we want to setup during the wizard.
  const [toggleFileIndexer, setToggleFileIndexer] = useState<boolean>(false);
  const [toggleAudioTranscription, setToggleAudioTranscription] =
    useState<boolean>(false);
  const handleOnChange = (e: SettingChangeEvents) => {
    if (e.settingName === "_.file-indexer") {
      setToggleFileIndexer(e.newValue as boolean);
    } else if (e.settingName === "_.audio-transcription") {
      setToggleAudioTranscription(e.newValue as boolean);
    }
  };

  const handleOnDone = async () => {
    await invoke("wizard_finished", {
      toggleAudioTranscription,
      toggleFileIndexer,
    }).catch((err) => console.error(err));
  };

  // When we reach the end
  useEffect(() => {
    if (stage == WizardStage.Done) {
      handleOnDone();
    }
  }, [stage]);

  let content = null;
  switch (stage) {
    case WizardStage.MenubarHelp:
      content = <MenubarHelpPage />;
      break;
    case WizardStage.DisplaySearchbarHelp:
      content = <DisplaySearchbarHelp />;
      break;
    case WizardStage.IndexCloud:
      content = <IndexCloudHelp />;
      break;
    case WizardStage.IndexBookmarks:
      content = <IndexBookmarksHelp />;
      break;
    case WizardStage.IndexWeb:
      content = <IndexWebHelp />;
      break;
    case WizardStage.IndexFiles:
      content = (
        <IndexFilesHelp
          toggleAudioTranscription={toggleAudioTranscription}
          toggleFileIndexer={toggleFileIndexer}
          onChange={handleOnChange}
        />
      );
      break;
    case WizardStage.Done:
      content = <WizardDone />;
      break;
  }

  return (
    <div>
      <div className="py-4 px-8 bg-neutral-800 h-screen text-center flex flex-col gap-4">
        {content}
      </div>
      <div className="mb-4 flex flex-row gap-4 justify-between absolute bottom-0 w-screen">
        {stage === WizardStage.Done ? (
          <progress className="mx-8 progress w-full" />
        ) : (
          <>
            {stage !== WizardStage.MenubarHelp ? (
              <Btn className="w-18" onClick={handleBack}>
                <ChevronLeftIcon className="w-8 ml-auto float-right" />
                Back
              </Btn>
            ) : (
              <div>&nbsp;</div>
            )}
            <Btn onClick={handleNext} className="ml-auto">
              <div>Next</div>
              <ChevronRightIcon className="w-8 ml-auto float-right" />
            </Btn>
          </>
        )}
      </div>
    </div>
  );
}

function WizardDone() {
  return <div>Saving settings...</div>;
}
