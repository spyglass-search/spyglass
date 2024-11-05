import { ReactNode } from "react";

interface Props {
  children: ReactNode;
}

export function KeyComponent({ children }: Props) {
  return (
    <div className="border border-neutral-500 rounded bg-neutral-400 text-black px-1 text-[8px] h-5 min-w-5 flex items-center font-semibold justify-center">
      {children}
    </div>
  );
}
