import { useRef } from "react";
import { SelectedLenses } from "./SelectedLens";

interface Props {
  selectedLenses: string[]
}

export function SearchInput({
  selectedLenses
}: Props) {
  const searchInput = useRef<HTMLInputElement>(null);
  const handleUpdateQuery = () => {};
  const handleKeyEvent = () => {};

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
        onKeyUp={handleKeyEvent}
        onClick={() => searchInput.current?.focus()}
        spellCheck={false}
        tabIndex={-1}
      />
    </div>
  )
}
