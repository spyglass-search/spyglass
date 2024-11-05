import { LensResult } from "../../bindings/LensResult";

interface LensResultItemProps {
  id: string;
  lens: LensResult;
  isSelected: boolean;
}

export function LensResultItem({ id, lens, isSelected }: LensResultItemProps) {
  return (
    <div
      id={id}
      className={` flex flex-col p-2 mt-2 text-white rounded scroll-mt-2 ${isSelected ? "bg-cyan-900" : "bg-neutral-800"}`}
    >
      <h2 className="text-2xl truncate py-1">{lens.label}</h2>
      <div className="text-sm leading-relaxed text-neutral-400">
        {lens.description}
      </div>
    </div>
  );
}
