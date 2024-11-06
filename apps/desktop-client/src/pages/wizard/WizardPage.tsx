import { ChevronLeftIcon, ChevronRightIcon } from "@heroicons/react/24/solid";
import { Btn } from "../../components/Btn";
import { useState } from "react";
import { MenubarHelpPage } from "./MenubarHelpPage";

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

  const content = <MenubarHelpPage />;

  return (
    <div className="py-4 px-8 bg-neutral-800 h-screen text-center flex flex-col gap-4">
      {stage}
      {content}
      <div className="mt-auto mb-2 flex flex-row gap-4 justify-between">
        {stage !== WizardStage.MenubarHelp ? (
          <Btn className="w-18" onClick={handleBack}>
            <ChevronLeftIcon className="w-8 ml-auto float-right" />
            Back
          </Btn>
        ) : null}
        <Btn onClick={handleNext} className="ml-auto">
          <div>Next</div>
          <ChevronRightIcon className="w-8 ml-auto float-right" />
        </Btn>
      </div>
    </div>
  );
}
