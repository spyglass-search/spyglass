import { UserActionDefinition } from "../../bindings/UserActionDefinition";

export const DEFAULT_ACTION: UserActionDefinition = {
  action: { OpenApplication: ["default", ""] },
  key_binding: "Enter",
  label: "Open with default app",
  status_msg: "OpenDefaultApplication",
};
