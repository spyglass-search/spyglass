import {
  BuildingLibraryIcon,
  Cog6ToothIcon,
  GlobeAltIcon,
  ShareIcon,
} from "@heroicons/react/24/solid";
import classNames from "classnames";
import { ReactNode } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { UserSettingsPage } from "./UserSettingsPage";
import { ConnectionManager } from "./ConnectionManager";

type Tab = "discover" | "library" | "connections" | "user";
interface Params {
  tab: Tab;
}

interface NavLinkProps {
  isSelected: boolean;
  children: ReactNode;
  onClick?: () => void;
}

function NavLink({ isSelected, children, onClick = () => {} }: NavLinkProps) {
  const styles = [
    "cursor-pointer",
    "flex-row",
    "flex",
    "hover:bg-neutral-700",
    "items-center",
    "p-2",
    "rounded",
    "w-full",
  ];

  return (
    <div
      className={classNames(styles, { "bg-neutral-700": isSelected })}
      onClick={onClick}
    >
      {children}
    </div>
  );
}

const MAIN_TABS: [string, Tab, ReactNode][] = [
  ["Discover", "discover", <GlobeAltIcon className="mr-2 w-4" />],
  ["My Library", "library", <BuildingLibraryIcon className="mr-2 w-4" />],
];

const CONFIG_TABS: [string, Tab, ReactNode][] = [
  ["Connections", "connections", <ShareIcon className="mr-2 w-4" />],
  ["User Settings", "user", <Cog6ToothIcon className="mr-2 w-4" />],
];

export function SettingsPage() {
  const params = useParams<keyof Params>();
  const nav = useNavigate();
  let tabContent = null;
  switch (params.tab) {
    case "connections":
      tabContent = <ConnectionManager />;
      break;
    case "discover":
      break;
    case "library":
      break;
    case "user":
      tabContent = <UserSettingsPage />;
      break;
  }

  const goto = (tab: Tab) => {
    nav(`/settings/${tab}`);
  };

  return (
    <div className="text-white flex h-screen">
      <div className="flex-col w-48 min-w-max bg-stone-900 p-4 top-0 left-0 z-40 sticky h-screen">
        <div className="mb-6">
          <div className="uppercase mb-2 text-xs text-gray-500 font-bold">
            Spyglass
          </div>
          <ul>
            {MAIN_TABS.map(([label, tab, icon]) => (
              <li key={label} className="mb-2">
                <NavLink
                  isSelected={params.tab === tab}
                  onClick={() => goto(tab)}
                >
                  {icon}
                  {label}
                </NavLink>
              </li>
            ))}
          </ul>
        </div>
        <div className="mb-6">
          <div className="uppercase mb-2 text-xs text-gray-500 font-bold">
            Configuration
          </div>
          <ul>
            {CONFIG_TABS.map(([label, tab, icon]) => (
              <li key={label} className="mb-2">
                <NavLink
                  isSelected={params.tab === tab}
                  onClick={() => goto(tab)}
                >
                  {icon}
                  {label}
                </NavLink>
              </li>
            ))}
          </ul>
        </div>
      </div>
      <div className="flex-col flex-1 h-screen overflow-y-auto bg-neutral-800">
        {tabContent}
      </div>
    </div>
  );
}
