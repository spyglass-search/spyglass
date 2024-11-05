import { KeyboardEvent, useEffect, useRef, useState } from "react";
import { invoke, listen } from "../../glue";
import { UserActionDefinition } from "../../bindings/UserActionDefinition";
import { LensResult } from "../../bindings/LensResult";
import { SearchResults } from "../../bindings/SearchResults";
import { SearchMeta } from "../../bindings/SearchMeta";
import { SearchResult } from "../../bindings/SearchResult";
import { SelectedLenses } from "./SelectedLens";
import { SearchStatus } from "./SearchStatus";
import { DocumentResultItem } from "./DocumentResultItem";
import { LensResultItem } from "./LensResultItem";
import { UserActionSettings } from "../../bindings/UserActionSettings";
import { ActionListButton, ActionsList } from "./ActionsList";

const LENS_SEARCH_PREFIX: string = "/";
const QUERY_DEBOUNCE_MS: number = 256;
const SEARCH_MIN_CHARS: number = 2;

enum ResultDisplay {
  None,
  Documents,
  Lenses,
}

// result_display: ResultDisplay::None,
// query_debounce: None,
// blur_timeout: None,
// action_settings: None,
// pressed_key: None,
// executed_key: None,
// executed_action: None,
// modifier: ModifiersState::empty(),
// show_actions: false,
// selected_action_idx: 0,
// action_menu_button_selected: false,

export function SearchPage() {
  const searchInput = useRef<HTMLInputElement>(null);
  const searchWrapperRef = useRef<HTMLDivElement>(null);

  const [selectedIdx, setSelectedIdx] = useState<number>(0);
  const [selectedLenses, setSelectedLenses] = useState<string[]>([]);

  const [docResults, setDocResults] = useState<SearchResult[]>([]);
  const [lensResults, setLensResults] = useState<LensResult[]>([]);
  const [resultMode, setResultMode] = useState<ResultDisplay>(
    ResultDisplay.None,
  );

  const [isThinking, setIsThinking] = useState<boolean>(false);
  const [showActions, setShowActions] = useState<boolean>(false);

  const [userActions, setUserActions] = useState<UserActionDefinition[]>([]);

  const [selectedActionIdx, setSelectedActionIdx] = useState<number>(0);
  const [searchMeta, setSearchMeta] = useState<SearchMeta | null>(null);

  const [query, setQuery] = useState<string>("");

  const requestResize = async () => {
    if (searchWrapperRef.current) {
      let height = searchWrapperRef.current.offsetHeight;
      await invoke("resize_window", { height });
    }
  };

  // Clear search queries & results
  const clearQuery = async () => {
    setQuery("");
    await clearResults();

    if (searchInput.current) {
      searchInput.current.value = "";
    }
    await requestResize();
  };

  // Clear search results
  const clearResults = async () => {
    setResultMode(ResultDisplay.None);
    setSelectedIdx(0);
    setDocResults([]);
    setLensResults([]);
    setShowActions(false);
    setSelectedActionIdx(0);
    setSearchMeta(null);
    await requestResize();
  };

  const moveSelectionUp = () => {
    if (showActions) {
    } else {
      // notihng to do
      if (resultMode === ResultDisplay.None) {
        return;
      }

      setSelectedIdx((idx) => (idx > 0 ? idx - 1 : idx));
    }
  };

  const moveSelectionDown = () => {
    if (showActions) {
    } else {
      let max = 0;
      if (resultMode === ResultDisplay.Documents) {
        max = docResults.length - 1;
      } else if (resultMode === ResultDisplay.Lenses) {
        max = lensResults.length - 1;
      }

      setSelectedIdx((idx) => {
        return idx === max ? max : idx + 1;
      });
    }
  };

  const handleKeyEvent = async (event: KeyboardEvent) => {
    if (event.type === "keydown") {
      const key = event.key;
      if (
        // ArrowXX: Prevent cursor from moving around
        key === "ArrowUp" ||
        key === "ArrowDown" ||
        // Tab: Prevent search box from losing focus
        key === "Tab"
      ) {
        event.preventDefault();
      }

      switch (event.key) {
        case "ArrowUp":
          moveSelectionUp();
          break;
        case "ArrowDown":
          moveSelectionDown();
          break;
        case "Enter":
          // do action or handle selection
          if (showActions) {
          } else {
            if (resultMode === ResultDisplay.Documents) {
              const selected = docResults[selectedIdx];
              await invoke("open_result", { url: selected.url });
              clearQuery();
            } else if (resultMode === ResultDisplay.Lenses) {
              const selected = lensResults[selectedIdx];
              setSelectedLenses((lenses) => [...lenses, selected.label]);
              clearQuery();
            }
          }
          break;
        case "Escape":
          // handle escape
          clearQuery();
          await invoke("escape");
          break;
        case "Backspace":
          // handle clearing lenses
          if (query.length === 0 && selectedLenses.length > 0) {
            setSelectedLenses([]);
          }
          break;
        default:
        // if (searchInput.current) {
        // setQuery(searchInput.current.value);
        // }
      }
    } else if (event.type === "keyup") {
      // handle keyup events.
    }
  };

  const handleUpdateQuery = () => {
    if (searchInput.current) {
      setQuery(searchInput.current.value);
    }
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
        setResultMode(ResultDisplay.Lenses);
        setLensResults(results);
        setIsThinking(false);
      } else if (query.length >= SEARCH_MIN_CHARS) {
        setIsThinking(true);
        // search docs
        const resp = await invoke<SearchResults>("search_docs", {
          query,
          lenses: selectedLenses,
        });
        setResultMode(ResultDisplay.Documents);
        setDocResults(resp.results);
        setSearchMeta(resp.meta);
        setIsThinking(false);
      }
    }, QUERY_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [query, selectedLenses]);

  useEffect(() => {
    requestResize();
  }, [docResults, lensResults]);

  // Scroll to the current selected result.
  useEffect(() => {
    const element = document.getElementById(`result-${selectedIdx}`);
    if (element) {
      element.scrollIntoView(true);
    }
  }, [selectedIdx]);

  useEffect(() => {
    clearQuery();
    clearResults();

    // get_action_list
    const fetchUserActions = async () => {
      const userActions = await invoke<UserActionSettings>(
        "load_action_settings",
      );
      setUserActions(userActions.actions);
    };

    const initialize = async () => {
      // Listen to refresh search results event
      await listen("RefreshSearchResults", () => {
        console.log("refreshsearchresults received");
      });
      await listen("ClearSearch", () => {
        console.log("ClearSearch received");
      });
      await listen("FocusWindow", () => {
        searchInput.current?.focus();
      });

      await fetchUserActions();
    };
    initialize().catch(console.error);
  }, []);

  return (
    <div
      ref={searchWrapperRef}
      className="relative overflow-hidden rounded-xl border-neutral-600 border"
      // onClick={(link.callback(|_| Msg::Focus))}
    >
      <div className="flex flex-nowrap w-full bg-neutral-800">
        <SelectedLenses lenses={selectedLenses} />
        <input
          ref={searchInput}
          id="searchbox"
          type="text"
          className="bg-neutral-800 text-white text-5xl py-3 overflow-hidden flex-1 outline-none active:outline-none focus:outline-none caret-white"
          placeholder="Search"
          onChange={handleUpdateQuery}
          onKeyDown={handleKeyEvent}
          onKeyUp={handleKeyEvent}
          // onClick={link.callback(|_| Msg::Focus)}
          spellCheck={false}
          tabIndex={-1}
        />
      </div>
      {resultMode === ResultDisplay.Documents ? (
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
      {resultMode === ResultDisplay.Lenses ? (
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
      <div className="flex flex-row w-full items-center bg-neutral-900 h-8 p-0">
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
          actions={userActions}
          selectedActionIdx={selectedActionIdx}
        />
      ) : null}
    </div>
  );
}
