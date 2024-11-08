import classNames from "classnames";
import {
  ArrowPathIcon,
  DocumentArrowDownIcon,
  EyeIcon,
  TagIcon,
  TrashIcon,
} from "@heroicons/react/24/solid";
import { Btn } from "./Btn";
import { BtnType } from "./_constants";

interface Props {
  author: string;
  categories?: string[];
  description: string;
  label: string;
  name: string;

  isInstalled?: boolean;
  isInstalling?: boolean;

  onCategoryClick?: (cat: string) => void;
  onInstall?: () => void;
  onUninstall?: () => void;
}

export function LibraryLens({
  author,
  label,
  name,
  description,
  categories = [],
  isInstalled = false,
  isInstalling = false,
  onCategoryClick = () => {},
  onInstall = () => {},
  onUninstall = () => {},
}: Props) {
  const styles = [
    "rounded-md",
    "bg-neutral-700",
    "p-4",
    "text-white",
    "shadow-md",
    "flex",
    "gap-4",
  ];

  const categoryTags = !isInstalled ? (
    <div className="mt-2 flex flex-row gap-2 flex-wrap text-xs items-center">
      <TagIcon className="w-4" />
      {categories.map((cat, idx) => (
        <div
          key={`category-${idx}`}
          className="bg-cyan-500 cursor-pointer text-white rounded px-1 py-0.5 hover:bg-cyan-600"
          onClick={() => onCategoryClick(cat)}
        >
          {cat}
        </div>
      ))}
    </div>
  ) : null;

  return (
    <div className={classNames(styles)}>
      <div className="flex flex-col flex-auto">
        <div className="text-base font-semibold">{label}</div>
        <div className="text-xs text-neutral-400">
          Crafted By:
          <a
            href={`https://github.com/${author}`}
            target="_blank"
            className="text-cyan-400"
          >
            {`@${author}`}
          </a>
        </div>
        <div className="text-sm text-neutral-400 mt-1">{description}</div>
        {categoryTags}
      </div>
      <LensActionBar
        name={name}
        isInstalled={isInstalled}
        isInstalling={isInstalling}
        onInstall={onInstall}
        onUninstall={onUninstall}
      />
    </div>
  );
}

interface LensActionBarProps {
  name: string;
  isInstalled: boolean;
  isInstalling: boolean;
  onInstall?: () => void;
  onUninstall?: () => void;
}

function LensActionBar({
  name,
  isInstalled,
  isInstalling,
  onInstall = () => {},
  onUninstall = () => {},
}: LensActionBarProps) {
  /// Create a view link to the lens directory HTML page.
  const viewLink = (lensName: string) => {
    const fmt = lensName.toLowerCase().replace("_", "-");
    return `https://lenses.spyglass.fyi/lenses/${fmt}`;
  };

  return (
    <div
      className={classNames(
        "flex",
        "flex-col",
        "flex-none",
        "place-content-start",
        "gap-2",
      )}
    >
      <Btn href={viewLink(name)} className="btn-sm text-sm">
        <EyeIcon className="w-3 mr-1" />
        Details
      </Btn>
      {isInstalled ? (
        <Btn
          type={BtnType.Danger}
          className="btn-sm text-sm"
          onClick={onUninstall}
        >
          <TrashIcon className="w-3" />
          Uninstall
        </Btn>
      ) : (
        <Btn
          disabled={isInstalling}
          type={BtnType.Success}
          className="btn-sm text-sm"
          onClick={onInstall}
        >
          {isInstalling ? (
            <ArrowPathIcon className="w-3 animate-spin" />
          ) : (
            <>
              <DocumentArrowDownIcon className="w-3 mr-1" />
              Install
            </>
          )}
        </Btn>
      )}
    </div>
  );
}
