import { useEffect } from "react";
import { LensResult } from "../../bindings/LensResult";
import { SearchResult } from "../../bindings/SearchResult";
import { ResultDisplayMode } from "./constants";
import { DocumentResultItem } from "./DocumentResultItem";
import { LensResultItem } from "./LensResultItem";

interface Props {
  docResults: SearchResult[];
  lensResults: LensResult[];
  displayMode: ResultDisplayMode;
  selectedIdx: number;
}

export function ResultListView({
  docResults,
  lensResults,
  displayMode,
  selectedIdx,
}: Props) {
  // Scroll to the current selected result whenever the selectedIdx changes.
  useEffect(() => {
    const element = document.getElementById(`result-${selectedIdx}`);
    if (element) {
      element.scrollIntoView(true);
    }
  }, [selectedIdx]);

  return (
    <div>
      {displayMode === ResultDisplayMode.Documents ? (
        <div className="overflow-y-auto overflow-x-hidden h-full max-h-[640px] bg-neutral-800 px-2 border-t border-neutral-600">
          <div className="w-full flex flex-col">
            {docResults.map((doc, idx) => (
              <DocumentResultItem
                key={doc.doc_id}
                id={`result-${idx}`}
                onClick={() => {}}
                result={doc}
                isSelected={selectedIdx === idx}
              />
            ))}
          </div>
        </div>
      ) : null}
      {displayMode === ResultDisplayMode.Lenses ? (
        <div className="overflow-y-auto overflow-x-hidden h-full max-h-[640px] bg-neutral-800 px-2 border-t border-neutral-600">
          <div className="w-full flex flex-col">
            {lensResults.map((lens, idx) => (
              <LensResultItem
                key={lens.name}
                id={`result-${idx}`}
                lens={lens}
                isSelected={selectedIdx === idx}
              />
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
}
