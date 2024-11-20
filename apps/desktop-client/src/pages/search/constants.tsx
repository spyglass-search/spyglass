import { UserActionDefinition } from "../../bindings/UserActionDefinition";

export const LENS_SEARCH_PREFIX: string = "/";
export const QUERY_DEBOUNCE_MS: number = 256;
export const SEARCH_MIN_CHARS: number = 2;

export const DEFAULT_ACTION: UserActionDefinition = {
  action: { OpenApplication: ["default", ""] },
  key_binding: "Enter",
  label: "Open with default app",
  status_msg: "OpenDefaultApplication",
};

export enum ResultDisplayMode {
  None,
  Documents,
  Lenses,
}
