import { KeyboardEvent, useCallback, useEffect, useState } from "react";
import { SearchInput } from "../search/SearchInput";
import { ResultListView } from "../search/ResultListView";
import {
  LENS_SEARCH_PREFIX,
  QUERY_DEBOUNCE_MS,
  ResultDisplayMode,
  SEARCH_MIN_CHARS,
} from "../search/constants";
import { SearchResult } from "../../bindings/SearchResult";
import { LensResult } from "../../bindings/LensResult";
import { invoke } from "../../glue";
import { SearchResults } from "../../bindings/SearchResults";

export function BigMode() {
  const [selectedLenses, setSelectedLenses] = useState<string[]>([]);
  const [query, setQuery] = useState<string>("");
  const [docResults, setDocResults] = useState<SearchResult[]>([]);
  const [lensResults, setLensResults] = useState<LensResult[]>([]);
  const [selectedIdx, setSelectedIdx] = useState<number>(0);

  const [isThinking, setIsThinking] = useState<boolean>(false);

  const [resultMode, setResultMode] = useState<ResultDisplayMode>(
    ResultDisplayMode.None,
  );

  // Clear search queries & results
  const clearQuery = useCallback(async () => {
    setQuery("");
    setResultMode(ResultDisplayMode.None);
    setSelectedIdx(0);
    setDocResults([]);
    setLensResults([]);
  }, []);

  const moveSelectionUp = () => {
    // notihng to do
    if (resultMode === ResultDisplayMode.None) {
      return;
    }
    setSelectedIdx((idx) => (idx > 0 ? idx - 1 : idx));
  };

  const moveSelectionDown = () => {
    let max = 0;
    if (resultMode === ResultDisplayMode.Documents) {
      max = docResults.length - 1;
    } else if (resultMode === ResultDisplayMode.Lenses) {
      max = lensResults.length - 1;
    }
    setSelectedIdx((idx) => (idx < max ? idx + 1 : max));
  };

  const handleEnter = async () => {
    // do action or handle selection
    if (resultMode === ResultDisplayMode.Documents) {
      const selected = docResults[selectedIdx];
      await invoke("open_result", { url: selected.url });
      clearQuery();
      await invoke("escape");
    } else if (resultMode === ResultDisplayMode.Lenses) {
      const selected = lensResults[selectedIdx];
      setSelectedLenses((lenses) => [...lenses, selected.label]);
      clearQuery();
    }
  };

  const handleKeyEvent = async (event: KeyboardEvent) => {
    switch (event.key) {
      case "ArrowUp":
        moveSelectionUp();
        break;
      case "ArrowDown":
        moveSelectionDown();
        break;
      case "Tab":
        // Handle tab completion for len search/results
        if (resultMode === ResultDisplayMode.Lenses) {
          const selected = lensResults[selectedIdx];
          setSelectedLenses((lenses) => [...lenses, selected.label]);
          clearQuery();
        }
        break;
    }
  };

  // when the query changes shoot it over to the server.
  useEffect(() => {
    if (query.length === 0) {
      clearQuery();
    }

    const timer = setTimeout(async () => {
      if (query.startsWith(LENS_SEARCH_PREFIX)) {
        setIsThinking(true);
        // search lenses.
        const trimmedQuery = query.substring(
          LENS_SEARCH_PREFIX.length,
          query.length,
        );
        const results = await invoke<LensResult[]>("search_lenses", {
          query: trimmedQuery,
        });
        setResultMode(ResultDisplayMode.Lenses);
        setLensResults(results);
        setIsThinking(false);
      } else if (query.length >= SEARCH_MIN_CHARS) {
        setIsThinking(true);
        // search docs
        const resp = await invoke<SearchResults>("search_docs", {
          query,
          lenses: selectedLenses,
          offset: 0,
        });
        setResultMode(ResultDisplayMode.Documents);
        setDocResults(resp.results);
        // setSearchMeta(resp.meta);
        setIsThinking(false);
      }
    }, QUERY_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [query, selectedLenses, clearQuery]);

  return (
    <div>
      <SearchInput
        selectedLenses={selectedLenses}
        setSelectedLenses={setSelectedLenses}
        query={query}
        setQuery={setQuery}
        onEnter={handleEnter}
        onKeyEvent={handleKeyEvent}
      />
      <ResultListView
        displayMode={resultMode}
        docResults={docResults}
        lensResults={lensResults}
        selectedIdx={selectedIdx}
      />
      {isThinking ? <progress className="progress w-full"></progress> : null}
    </div>
  );
}
