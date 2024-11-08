import { ChangeEvent, useEffect, useRef, useState } from "react";
import { Header } from "./Header";
import { ArrowPathIcon } from "@heroicons/react/24/solid";
import { invoke, listen } from "../../glue";
import { InstallableLens } from "../../bindings/InstallableLens";
import { LibraryLens } from "../../components/LibraryLens";

type CategoryCounts = { [k: string]: number };

export function Discover() {
  const categoryRef = useRef<HTMLSelectElement>(null);

  const [categoryFilter, setCategoryFilter] = useState<string>("ALL");
  const [nameFilter, setNameFilter] = useState<string>("");

  const [isLoading, setIsLoading] = useState<boolean>(true);
  const [categoryList, setCategoryList] = useState<CategoryCounts>({});
  const [installable, setInstallable] = useState<InstallableLens[]>([]);
  const [filteredList, setFilteredList] = useState<InstallableLens[]>([]);

  const [installing, setInstalling] = useState<string[]>([]);

  const handleCategoryClick = (category: string) => {
    if (categoryRef.current) {
      categoryRef.current.value = category;
      setCategoryFilter(category);
    }
  };

  const handleCategoryFilter = () => {
    if (categoryRef.current) {
      setCategoryFilter(categoryRef.current.value);
    }
  };

  const handleNameFilter = (event: ChangeEvent<HTMLInputElement>) => {
    setNameFilter(event.currentTarget.value.toLowerCase());
  };

  const handleInstall = async (lens: InstallableLens) => {
    if (installing.includes(lens.name)) {
      return;
    }

    setInstalling((list) => {
      const updated = [...list, lens.name];
      return updated;
    });

    await invoke("install_lens", { name: lens.name });
  };

  const handleRefresh = async () => {
    const lenses = await invoke<InstallableLens[]>(
      "list_installable_lenses",
    ).finally(() => setIsLoading(false));

    const categories: CategoryCounts = {};
    lenses.forEach((lens) => {
      lens.categories.forEach((cat) => {
        if (cat in categories) {
          categories[cat] += 1;
        } else {
          categories[cat] = 1;
        }
      });
    });

    setCategoryList(categories);
    setInstallable(lenses);
  };

  useEffect(() => {
    const filtered: InstallableLens[] = installable.flatMap((lens) => {
      let skip = true;
      if (categoryFilter !== "ALL") {
        skip = !lens.categories.includes(categoryFilter);
      } else {
        skip = false;
      }

      if (!skip && nameFilter.length > 0) {
        skip = !lens.name.toLowerCase().includes(nameFilter);
      }

      return skip ? [] : [lens];
    });
    setFilteredList(filtered);
  }, [categoryFilter, nameFilter, installable]);

  useEffect(() => {
    const init = async () => {
      await handleRefresh();
      return await listen("RefreshDiscover", async () => {
        await handleRefresh();
      });
    };

    const unlisten = init();
    return () => {
      (async () => {
        await unlisten.then((fn) => fn());
      })();
    };
  }, []);
  return (
    <div>
      <Header label={"Discover"}>
        <select
          className="w-40 rounded p-2 text-sm form-input placeholder-neutral-400 bg-neutral-700 border-neutral-800"
          ref={categoryRef}
          onChange={handleCategoryFilter}
        >
          <option value="ALL">{"All"}</option>
          {Object.keys(categoryList)
            .sort()
            .map((label) => (
              <option
                key={label}
                value={label}
              >{`${label} (${categoryList[label]})`}</option>
            ))}
        </select>
        <input
          type="text"
          placeholder="search installable lenses"
          className="w-full rounded p-2 text-sm form-input placeholder-neutral-400 bg-neutral-700 border-neutral-800"
          onChange={handleNameFilter}
        />
      </Header>
      <div className="p-4 flex flex-col gap-2">
        {isLoading ? (
          <div className="flex justify-center">
            <div className="p-16 flex flex-col gap-4 items-center">
              <ArrowPathIcon className="w-16 animate-spin" />
              Fetching lens list
            </div>
          </div>
        ) : (
          filteredList.map((lens) => (
            <LibraryLens
              key={lens.name}
              author={lens.author}
              categories={lens.categories}
              description={lens.description}
              label={lens.label}
              name={lens.name}
              isInstalling={installing.includes(lens.name)}
              onCategoryClick={handleCategoryClick}
              onInstall={() => handleInstall(lens)}
            />
          ))
        )}
      </div>
    </div>
  );
}
