import React, { KeyboardEvent, useEffect, useRef } from "react";
import { SelectedLenses } from "./SelectedLens";
import { listen } from "@tauri-apps/api/event";

interface Props {
  selectedLenses: string[];
  setSelectedLenses: React.Dispatch<React.SetStateAction<string[]>>;
  query: string;
  setQuery: React.Dispatch<React.SetStateAction<string>>;
  // Event handlers.
  onEnter?: (event: KeyboardEvent) => void;
  onKeyEvent?: (event: KeyboardEvent) => void;
}

export function SearchInput({
  selectedLenses,
  setSelectedLenses,
  query,
  setQuery,
  onEnter = () => {},
  onKeyEvent = () => {},
}: Props) {
  const searchInput = useRef<HTMLInputElement>(null);
  const handleUpdateQuery = () => {
    if (searchInput.current) {
      setQuery(searchInput.current.value);
    }
  };

  const handleKeyEvent = async (event: KeyboardEvent) => {
    const key = event.key;
    // ArrowXX: Prevent cursor from moving around
    // Tab: Prevent search box from losing focus
    if (["ArrowUp", "ArrowDown", "Tab"].includes(key)) {
      event.preventDefault();
    }

    switch (event.key) {
      case "Backspace":
        // handle clearing lenses
        if (query.length === 0 && selectedLenses.length > 0) {
          setSelectedLenses([]);
        }
        break;
      case "Enter":
        return onEnter(event);
      default:
        return onKeyEvent(event);
    }
  };

  useEffect(() => {
    const initialize = async () => {
      await listen("FocusWindow", () => {
        searchInput.current?.focus();
      });
    };

    initialize().catch(console.error);
  }, []);

  return (
    <div className="flex flex-nowrap w-full bg-neutral-800">
      <SelectedLenses lenses={selectedLenses} />
      <input
        ref={searchInput}
        id="searchbox"
        type="text"
        className="bg-neutral-800 text-white text-5xl py-3 overflow-hidden flex-1 border-none caret-white active:outline-none focus-visible:outline-none focus:outline-none"
        placeholder="Search"
        onChange={handleUpdateQuery}
        onKeyDown={handleKeyEvent}
        onClick={() => searchInput.current?.focus()}
        value={query}
        spellCheck={false}
        tabIndex={-1}
      />
    </div>
  );
}
