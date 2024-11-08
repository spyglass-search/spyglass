import { SettingChangeEvents } from "../_constants";

export interface FormFieldProps {
  name: string;
  value: string | boolean | string[];
  className?: string;
  onChange?: (e: SettingChangeEvents) => void;
}
