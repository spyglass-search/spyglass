import { KeyboardEvent, useCallback, useEffect, useRef, useState } from "react";
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
import { invoke, listen } from "../../glue";
import { SearchResults } from "../../bindings/SearchResults";
import classNames from "classnames";
import { ChatMessage } from "../../bindings/ChatMessage";
import { ArrowPathIcon, ExclamationTriangleIcon } from "@heroicons/react/24/solid";
import { Btn } from "../../components/Btn";
import { BtnType } from "../../components/_constants";
import { ChatStream } from "../../bindings/ChatStream";

enum Tab {
  Chat,
  Search,
}

export function BigMode() {
  const [activeTab, setActiveTab] = useState<Tab>(Tab.Search);

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
    <div className="h-screen flex flex-col">
      <div role="tablist" className="tabs tabs-boxed">
        <a role="tab"
          className={classNames("tab", {"tab-active": activeTab === Tab.Search})}
          onClick={() => setActiveTab(Tab.Search)}
        >
          Search
        </a>
        <a
          role="tab"
          className={classNames("tab", {"tab-active": activeTab === Tab.Chat})}
          onClick={() => setActiveTab(Tab.Chat)}
        >
          Chat
        </a>
      </div>
      {activeTab == Tab.Search ? (
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
      ) : null}
      {activeTab == Tab.Chat ? (
        <AskClippy />
      ) : null}
    </div>
  );
}

interface ChatLogProps {
  history: ChatMessage[]
}

function ChatLogItem({
  chat,
  isStreaming = false
}: { chat: ChatMessage, isStreaming?: boolean }) {
  const isUser = chat.role === "user";

  const icon = chat.role === "assistant" ? "🤖" : "🧙‍♂️";
  return (
    <div className="border-t border-t-neutral-700 p-4 text-sm text-white items-center flex flex-row gap-4 animate-fade-in">
      <div className={classNames(
          "flex", "flex-none", "border", "border-cyan-600", "w-[48px]", "h-[48px]", "rounded-full", "items-center",
          { "order-1": isUser }
      )}>
        <div className="text-lg mx-auto">{icon}</div>
      </div>
      <div className={classNames("grow", { "text-left": !isUser, "text-right": isUser })}>
        {chat.content}
      </div>
    </div>
  );
}

function ChatLog({ history }: ChatLogProps) {
  return (
    <div>
      {history.map((chat, idx) => <ChatLogItem key={`chat-log-${idx}`} chat={chat} />)}
    </div>
  );
}

function AskClippy() {
  const clippyInput = useRef<HTMLTextAreaElement>(null);

  const [isStreaming, setIsStreaming] = useState<boolean>(false);
  const [stream, setStream] = useState<string>('');

  const [history, setHistory] = useState<ChatMessage[]>([
    { role: "user", content: "hi what's your name?" },
    { role: "assistant", content: "test" }
  ]);
  const [status, setStatus] = useState<string>('');

  const handleChatEvent = (event: ChatStream) => {
    if (event.type == "LoadingPrompt") {
      setStatus("Loading prompt...");
    } else if (event.type == "Token") {
      setStream(str => str + event.content);
    } else if (event.type == "ChatDone") {
      setIsStreaming(false);
      setHistory(hist => ([...hist, {
        role: "assistant",
        content: stream,
      }]));
      setStream('');
    }
  };

  const handleAskClippy = async (prompt: string) => {
    let currentCtxt: ChatMessage[] = [...history, {
      role: "user",
      content: prompt
    }];
    setHistory(currentCtxt);
    setIsStreaming(true);
    await invoke("ask_clippy", { session: { messages: currentCtxt }});
  };

  const handleQuerySubmission = () => {};

  const clearHistory = () => {
    setHistory([]);
  };

  useEffect(() => {
    const init = async () => {
      await listen<ChatStream>("ChatEvent", (event) => handleChatEvent(event.payload));
    };

    init();
  }, []);

  return (
    <div className="flex flex-col grow bg-neutral-800 text-white">
      <div className="flex flex-col grow place-content-end min-h-[128px]">
        <div className="flex flex-col overflow-y-scroll">
          <ChatLog history={history} />
          { isStreaming ? (
            <ChatLogItem chat={{ role: "assistant", content: stream ?? status }} isStreaming={isStreaming} />
          ) : null}
        </div>
      </div>
      <div>
        <div className="bg-neutral-700 px-4 py-2 text-sm text-neutral-400 flex flex-row items-center gap-4">
          <ExclamationTriangleIcon className="w-6 text-yellow-400" />
          <div>
            <a
              className="cursor-help underline font-semibold text-cyan-500"
              onClick={() => handleAskClippy("what is a language model?")}
            >
              LLMs
            </a>
            (the tech behind this) are still experimental and responses may be inaccurate.
          </div>
        </div>
        <div className="p-4">
          <div className="flex flex-row gap-4 items-center">
            <textarea
              ref={clippyInput}
              rows={2}
              placeholder="what is the difference between an alpaca & llama?"
              className="text-base bg-neutral-800 text-white flex-1 outline-none active:outline-none focus:outline-none caret-white border-b-2 border-neutral-600 rounded"
            />
            <div className="flex flex-col gap-1">
              <Btn disabled={isStreaming} className="btn-sm" type={BtnType.Primary} onClick={() => handleQuerySubmission()}>
              {isStreaming ? (
                <div><ArrowPathIcon className="animate-spin w-4" /></div>
              ) : (
                <div>Ask</div>
              )}
              </Btn>
              <Btn onClick={clearHistory} className="btn-sm">Clear</Btn>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
