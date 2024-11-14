import { useRef, useState } from "react";
import { FormFieldProps } from "./_constants";
import classNames from "classnames";

export function Text({
  value,
  className,
  onChange = () => {},
}: FormFieldProps) {
  const ref = useRef<HTMLInputElement>(null);
  const [text, setText] = useState<string>(value.toString());
  const styles = [
    "input",
    "input-bordered",
    "text-sm",
    "bg-stone-700",
    "border-stone-800",
    "text-white",
  ];

  const handleChange = () => {
    if (ref.current) {
      setText(ref.current.value);
      onChange({
        oldValue: value.toString(),
        newValue: ref.current.value,
      });
    }
  };

  return (
    <input
      ref={ref}
      spellCheck={false}
      onChange={handleChange}
      value={text}
      type="text"
      className={classNames(styles, className)}
    />
  );
}
