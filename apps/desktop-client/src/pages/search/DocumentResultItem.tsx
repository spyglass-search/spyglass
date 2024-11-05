import { MagnifyingGlassCircleIcon, StarIcon } from "@heroicons/react/24/solid";
import { SearchResult } from "../../bindings/SearchResult";
import { ReactNode } from "react";
import { FolderIcon } from "@heroicons/react/24/solid";
import { FileExtIcon } from "../../components/FileExtIcon";
import { ConnectionIcon } from "../../components/ConnectionIcon";

interface Props {
  id: string;
  onClick: () => void;
  result: SearchResult;
  isSelected: boolean;
}

export function DocumentResultItem({ id, onClick, result, isSelected }: Props) {
  const url = new URL(result.crawl_uri);
  const styles = [
    "flex",
    "flex-row",
    "gap-4",
    "rounded",
    "py-2",
    "pr-2",
    "mt-2",
    "text-white",
    "cursor-pointer",
    "active:bg-cyan-900",
    "scroll-mt-2",
    isSelected ? "bg-cyan-900" : "bg-neutral-800",
  ];

  return (
    <a id={id} className={styles.join(" ")} onClick={onClick}>
      <div className="mt-1 flex flex-none pl-6 pr-2">
        <div className="relative flex-none bg-neutral-700 rounded h-12 w-12 items-center">
          <DocumentIcon result={result} />
        </div>
      </div>
      <div className="grow">
        <div className="text-xs text-cyan-500">{url.hostname}</div>
        <h2 className="text-base truncate font-semibold w-[30rem]">
          {result.title}
        </h2>
        <div
          className="text-sm leading-relaxed text-neutral-400 max-h-10 overflow-hidden"
          dangerouslySetInnerHTML={{ __html: result.description }}
        />
        <DocumentMeta result={result} />
      </div>
    </a>
  );
}

function DocumentIcon({ result }: { result: SearchResult }) {
  const url = new URL(result.crawl_uri);
  const iconStyles = ["w-8", "h-8", "m-auto", "mt-2"];

  // Third-party connections like Reddit/Gmail/etc.
  if (url.protocol === "api") {
    return (
      <ConnectionIcon
        connection={url.hostname}
        className={iconStyles.join(" ")}
      />
    );
  } else if (url.protocol === "file") {
    const isDirectory = result.tags
      .map(
        ([label, value]) =>
          label.toLowerCase() === "type" && value.toLowerCase() === "directory",
      )
      .reduce((prev, cur) => prev || cur, false);

    return isDirectory ? (
      <FolderIcon className="w-8 bg-color-white m-auto mt-2" />
    ) : (
      <FileExtIcon className={iconStyles.join(" ")} filePath={result.title} />
    );
  }

  return (
    <img
      className={iconStyles.join(" ")}
      alt="Website"
      src={`https://icons.duckduckgo.com/ip3/${url.hostname}.ico`}
    />
  );
}

function DocumentMeta({ result }: { result: SearchResult }) {
  const priorityTags: ReactNode[] = [];
  const normalTags: ReactNode[] = [];

  const types = result.tags.flatMap(([label, value]) =>
    label.toLowerCase() === "type" ? [value] : [],
  );

  result.tags.forEach(([label, value]) => {
    const tag = label.toLowerCase();
    if (tag === "source" || tag === "mimetype") {
      return;
    }

    if (
      types.findIndex((type) => type === "repository") &&
      tag === "repository"
    ) {
      return;
    }

    const tagComponent = (
      <DocumentTag key={`${label}:${value}`} label={label} value={value} />
    );
    if (tag === "favorited") {
      priorityTags.push(tagComponent);
    } else {
      normalTags.push(tagComponent);
    }
  });

  return (
    <div className="text-xs place-items-center flex flex-row flex-wrap gap-2 text-cyan-500 py-0.5 mt-1.5">
      {[...priorityTags, ...normalTags]}
    </div>
  );
}

function DocumentTag({ label, value }: { label: string; value: string }) {
  if (label.toLowerCase() === "favorited") {
    return (
      <div className="items-center">
        <StarIcon className="w-4 text-yellow-500" />
      </div>
    );
  }

  const tagLabel =
    label === "lens" ? (
      <MagnifyingGlassCircleIcon className="w-4" />
    ) : (
      <>{label}</>
    );

  return (
    <div className="flex flex-row rounded border border-neutral-600 gap-1 py-0.5 px-1 text-xs text-white">
      <div className="font-bold text-cyan-600">{tagLabel}</div>
      <div>{value}</div>
    </div>
  );
}
