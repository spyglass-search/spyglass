interface Props {
  lenses: string[];
}

export function SelectedLenses({ lenses }: Props) {
  if (lenses.length === 0) {
    return null;
  }

  return (
    <ul className="flex bg-neutral-800 gap-2 items-center mx-3">
      {lenses.map((lens) => (
        <li
          key={lens}
          className="flex bg-cyan-700 rounded-lg px-2 py-1 text-4xl text-white"
        >
          {lens}
        </li>
      ))}
    </ul>
  );
}
