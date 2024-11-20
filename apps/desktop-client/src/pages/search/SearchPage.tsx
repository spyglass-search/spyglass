import { KeyboardEvent, useCallback, useEffect, useRef, useState } from "react";
import { invoke, listen } from "../../glue";
import { UserActionDefinition } from "../../bindings/UserActionDefinition";
import { LensResult } from "../../bindings/LensResult";
import { SearchResults } from "../../bindings/SearchResults";
import { SearchMeta } from "../../bindings/SearchMeta";
import { SearchResult } from "../../bindings/SearchResult";
import { SearchStatus } from "./SearchStatus";
import { UserActionSettings } from "../../bindings/UserActionSettings";
import { ActionListButton, ActionsList } from "./ActionsList";
import {
  DEFAULT_ACTION,
  LENS_SEARCH_PREFIX,
  QUERY_DEBOUNCE_MS,
  ResultDisplayMode,
  SEARCH_MIN_CHARS,
} from "./constants";
import Handlebars from "handlebars";
import { ContextActions } from "../../bindings/ContextActions";
import { includeAction, resultToTemplate } from "./utils";
import { CustomTitleBar } from "../../components/CustomTitleBar";
import { SearchInput } from "./SearchInput";
import { ResultListView } from "./ResultListView";

export function SearchPage() {
  const searchWrapperRef = useRef<HTMLDivElement>(null);

  const [selectedIdx, setSelectedIdx] = useState<number>(0);
  const [selectedLenses, setSelectedLenses] = useState<string[]>([]);

  const [docResults, setDocResults] = useState<SearchResult[]>([]);
  const [lensResults, setLensResults] = useState<LensResult[]>([]);
  const [resultMode, setResultMode] = useState<ResultDisplayMode>(
    ResultDisplayMode.None,
  );

  const [isThinking, setIsThinking] = useState<boolean>(false);
  const [showActions, setShowActions] = useState<boolean>(false);

  const [userActions, setUserActions] = useState<UserActionDefinition[]>([]);
  const [currentContextActions, setCurrentContextActions] = useState<
    UserActionDefinition[]
  >([]);
  const [contextActions, setContextActions] = useState<ContextActions[]>([]);

  const [selectedActionIdx, setSelectedActionIdx] = useState<number>(0);
  const [searchMeta, setSearchMeta] = useState<SearchMeta | null>(null);

  const [query, setQuery] = useState<string>("");

  const requestResize = async () => {
    if (searchWrapperRef.current) {
      const height = searchWrapperRef.current.offsetHeight;
      await invoke("resize_window", { height });
    }
  };

  // Clear search results
  const clearResults = useCallback(async () => {
    setResultMode(ResultDisplayMode.None);
    setSelectedIdx(0);
    setDocResults([]);
    setLensResults([]);
    setShowActions(false);
    setSelectedActionIdx(0);
    setSearchMeta(null);
    await requestResize();
  }, []);

  // Clear search queries & results
  const clearQuery = useCallback(async () => {
    setQuery("");
    await clearResults();
  }, [clearResults]);

  const moveSelectionUp = () => {
    if (showActions) {
      // Actions start at idx 1 since the default action (open) is always 0
      setSelectedActionIdx((idx) => (idx > 0 ? idx - 1 : idx));
    } else {
      // notihng to do
      if (resultMode === ResultDisplayMode.None) {
        return;
      }
      setSelectedIdx((idx) => (idx > 0 ? idx - 1 : idx));
    }
  };

  const moveSelectionDown = () => {
    if (showActions) {
      // default + number of actions
      const max = 1 + (currentContextActions.length - 1);
      setSelectedActionIdx((idx) => (idx < max ? idx + 1 : max));
    } else {
      let max = 0;
      if (resultMode === ResultDisplayMode.Documents) {
        max = docResults.length - 1;
      } else if (resultMode === ResultDisplayMode.Lenses) {
        max = lensResults.length - 1;
      }
      setSelectedIdx((idx) => (idx < max ? idx + 1 : max));
    }
  };

  const handleEnter = async () => {
    // do action or handle selection
    if (showActions) {
      // handle whichever action is selected.
      const action =
        selectedActionIdx === 0
          ? DEFAULT_ACTION
          : currentContextActions[selectedActionIdx - 1];
      handleSelectedAction(action);
    } else {
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
      case "Escape":
        // Close action menu if we're in it.
        if (showActions) {
          setShowActions(false);
          // otherwise close the window.
        } else {
          clearQuery();
          await invoke("escape");
        }
        break;
      case "Tab":
        // Handle tab completion for len search/results
        if (resultMode === ResultDisplayMode.Lenses) {
          const selected = lensResults[selectedIdx];
          setSelectedLenses((lenses) => [...lenses, selected.label]);
          clearQuery();
          // Jump to action menu
        } else if (resultMode === ResultDisplayMode.Documents) {
          setShowActions(true);
        }
        break;
    }
  };

  const handleSelectedAction = async (action: UserActionDefinition) => {
    console.debug("handling action: ", action);
    // Get the context for the action execution
    const selectedDoc = docResults[selectedIdx];
    // open in application
    if ("OpenApplication" in action.action) {
      const url = selectedDoc.url;
      const [app] = action.action.OpenApplication;
      await invoke("open_result", { url, application: app });
      // Open url
    } else if ("OpenUrl" in action.action) {
      const template = Handlebars.compile(action.action.OpenUrl);
      const selectedResultTemplate = resultToTemplate(selectedDoc);

      await invoke("open_result", {
        url: template(selectedResultTemplate),
        application: null,
      });
    } else if ("CopyToClipboard" in action.action) {
      const selectedResultTemplate = resultToTemplate(selectedDoc);
      const template = Handlebars.compile(action.action.CopyToClipboard);

      await invoke("copy_to_clipboard", {
        txt: template(selectedResultTemplate),
      });
    }

    setShowActions(false);
  };

  // when the query changes shoot it over to the server.
  useEffect(() => {
    if (query.length === 0) {
      clearResults();
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
        });
        setResultMode(ResultDisplayMode.Documents);
        setDocResults(resp.results);
        setSearchMeta(resp.meta);
        setIsThinking(false);
      }
    }, QUERY_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [query, selectedLenses, clearResults]);

  useEffect(() => {
    const newActions = [...userActions];
    if (docResults.length > selectedIdx && contextActions.length > 0) {
      const selected = docResults[selectedIdx];

      for (const action of contextActions) {
        if (includeAction(action, selected)) {
          for (const actionDefinition of action.actions) {
            newActions.push(actionDefinition);
          }
        }
      }
    }
    setCurrentContextActions(newActions);
  }, [selectedIdx, contextActions, userActions, docResults]);

  useEffect(() => {
    requestResize();
  }, [docResults, lensResults]);

  useEffect(() => {
    // get_action_list
    const fetchUserActions = async () => {
      const userActions = await invoke<UserActionSettings>(
        "load_action_settings",
      );
      setUserActions(userActions.actions);
      setContextActions(userActions.context_actions);
    };

    const initialize = async () => {
      // Listen to refresh search results event
      await listen("RefreshSearchResults", () => {
        console.log("refreshsearchresults received");
      });
      await listen("ClearSearch", () => {
        clearResults();
      });
      await fetchUserActions();
    };

    initialize().catch(console.error);
  }, [clearResults]);

  return (
    <div
      ref={searchWrapperRef}
      className="relative overflow-clip rounded-xl bg-transparent"
    >
      <CustomTitleBar />
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
      <div
        data-tauri-drag-region
        className="flex flex-row w-full items-center bg-neutral-900 h-8 p-0"
      >
        <SearchStatus meta={searchMeta} isThinking={isThinking} />
        {searchMeta ? (
          <ActionListButton
            isActive={showActions}
            onClick={() => setShowActions((val) => !val)}
          />
        ) : null}
      </div>
      {showActions ? (
        <ActionsList
          actions={currentContextActions}
          selectedActionIdx={selectedActionIdx}
          onClick={handleSelectedAction}
        />
      ) : null}
    </div>
  );
}
