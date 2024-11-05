import { KeyboardEvent, useEffect, useRef, useState } from "react";
import { invoke, listen } from "../../glue";
import { UserActionDefinition } from "../../bindings/UserActionDefinition";
import { LensResult } from "../../bindings/LensResult";
import { SearchResults } from "../../bindings/SearchResults";
import { SearchMeta } from "../../bindings/SearchMeta";
import { SearchResult } from "../../bindings/SearchResult";

const LENS_SEARCH_PREFIX: string = '/';
const QUERY_DEBOUNCE_MS: number = 256;
const SEARCH_MIN_CHARS: number = 2;

enum ResultDisplay {
  None,
  Documents,
  Lenses,
}

interface SelectedLensProps {
  lens: string[];
}

function SelectedLens({ lens }: SelectedLensProps) {
  return <div>{lens}</div>;
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
  const [selectedLens, setSelectedLens] = useState<string[]>([]);

  const [docResults, setDocResults] = useState<SearchResult[]>([]);
  const [lensResults, setLensResults] = useState<LensResult[]>([]);
  const [resultMode, setResultMode] = useState<ResultDisplay>(ResultDisplay.None);

  const [_showActions, setShowActions] = useState<boolean>(false);
  const [_selectedActionIdx, setSelectedActionIdx] = useState<number>(0);
  const [_searchMeta, setSearchMeta] = useState<SearchMeta | null>(null);

  const [query, setQuery] = useState<string>("");
  // const [isSearching, setIsSearching] = useState<boolean>(false);

  const requestResize = async () => {
    if (searchWrapperRef.current) {
      console.debug(`resizing window to: ${searchWrapperRef.current.offsetHeight}`);
      // let height = searchWrapperRef.current.offsetHeight;
      // await invoke("resize_window", { height });
    }
  };
  const clearFilters = () => {
    setSelectedLens([]);
  };

  const clearQuery = async () => {
    setSelectedIdx(0);
    setDocResults([]);
    setLensResults([]);
    setShowActions(false);
    setSelectedActionIdx(0);
    setSearchMeta(null);
    setQuery("");
    if (searchInput.current) {
      searchInput.current.value = "";
    }
    await requestResize();
  };

  const clearResults = () => {
    setSelectedIdx(0);
    setDocResults([]);
    setLensResults([]);
    setShowActions(false);
    setSelectedActionIdx(0);
    setSearchMeta(null);
  };

  const moveSelectionDown = () => {
    if (_showActions) {} else {
      let max = 0;
      if(resultMode === ResultDisplay.Documents) {
        max = docResults.length;
      } else if (resultMode === ResultDisplay.Lenses) {
        max = lensResults.length;
      }

      setSelectedIdx((idx) => {
        return idx === max ? max : idx + 1;
      });
    }
  };

  const handleKeyEvent = (event: KeyboardEvent) => {
    if (event.type === "keydown") {
      let key = event.key;
      if (
        // ArrowXX: Prevent cursor from moving around
        key === "ArrowUp"
        || key === "ArrowDown"
        // Tab: Prevent search box from losing focus
        || key === "Tab"
      ) {
        event.preventDefault();
      }

      switch(event.key) {
        case "ArrowUp":
          setSelectedIdx(idx => idx > 0 ? idx - 1 : idx); break;
        case "ArrowDown":
          moveSelectionDown(); break;
        case "Enter":
            // do action or handle selection
            break;
        case "Escape":
            // handle escape
            clearQuery();
            break;
        case "Backspace":
            // handle clearing lenses
            if(query.length === 0 && selectedLens.length > 0){
              setSelectedLens([]);
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
      setQuery(searchInput.current.value)
    }
  };

  // when the query changes shoot it over to the server.
  useEffect(() => {
    const timer = setTimeout(async () => {
      if (query.startsWith(LENS_SEARCH_PREFIX)) {
        // search lenses.
        let trimmedQuery = query.substring(LENS_SEARCH_PREFIX.length, query.length);
        let results = await invoke<LensResult[]>("search_lenses", { query: trimmedQuery });
        setResultMode(ResultDisplay.Lenses);
        setLensResults(results);
      } else if (query.length >= SEARCH_MIN_CHARS) {
        // search docs
        let resp = await invoke<SearchResults>("search_docs", { query });
        setResultMode(ResultDisplay.Documents);
        setDocResults(resp.results);
        setSearchMeta(resp.meta);
      }
    }, QUERY_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [query]);

  useEffect(() => {
    clearFilters();
    clearQuery();
    clearResults();

    // get_action_list
    const fetchUserActions = async () => {
      const userActions = await invoke<UserActionDefinition[]>(
        "load_action_settings",
      );
      console.log(userActions);
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
        <SelectedLens lens={selectedLens} />
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
      {docResults.length > 0 || lensResults.length > 0 ? (
        <div className="overflow-y-auto overflow-x-hidden h-full max-h-[640px] bg-neutral-800 px-2 border-t border-neutral-600">
          <div className="w-full flex flex-col">
          {lensResults.map((lens, idx) => (
            <LensResultItem key={lens.name} lens={lens} isSelected={selectedIdx === idx} />
          ))}
          </div>
        </div>
      ) : null
      }
      <div className="flex flex-row w-full items-center bg-neutral-900 h-8 p-0">
        <div className="grow text-neutral-500 text-sm pl-3 flex flex-row items-center">
          {"search_meta"}
        </div>
        {/* <ActionListBtn
                    show={self.search_meta.is_some()}
                    is_active={self.action_menu_button_selected || self.show_actions}
                    onClick={link.callback(|_| Msg::ToggleShowActions)}
                /> */}
      </div>
      {/* <ActionsList
                show={self.show_actions}
                actions={self.get_action_list()}
                selected_action={self.selected_action_idx}
                onClick={link.callback(Msg::UserActionSelected)}
            /> */}
    </div>
  );
}

interface LensResultItemProps {
  lens: LensResult;
  isSelected: boolean;
}

function LensResultItem({ lens, isSelected }: LensResultItemProps) {
  return (
    <div className={` flex flex-col p-2 mt-2 text-white rounded scroll-mt-2 ${isSelected ? "bg-cyan-900" : "bg-neutral-800"}`}>
      <h2 className="text-2xl truncate py-1">
        {lens.label}
      </h2>
      <div className="text-sm leading-relaxed text-neutral-400">
        {lens.description}
      </div>
    </div>
  );
}