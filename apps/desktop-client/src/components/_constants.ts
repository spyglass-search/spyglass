// NOTE: Fixes a linting issue by keeping constants in its own file.
// Fast refresh only works when a file only exports components. Use a new file to
// share constants or functions between components  react-refresh/only-export-components

export enum BtnType {
  Default,
  Borderless,
  Danger,
  Success,
  Primary,
}

export type SettingChangeEvents =
  | SettingChangeEvent<boolean>
  | SettingChangeEvent<string>
  | SettingChangeEvent<string[]>;

export interface SettingChangeEvent<T> {
  settingName: string;
  oldValue: T;
  newValue: T;
  restartRequired: boolean;
}
