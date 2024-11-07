import { useEffect, useState } from "react";
import { FileExtIcon } from "../../components/FileExtIcon";
import { FormElement } from "../../components/FormElement";
import { SettingOpts } from "../../bindings/SettingOpts";
import { invoke } from "../../glue";
import { DefaultIndices } from "../../bindings/DefaultIndices";
import { SettingChangeEvents } from "../../components/_constants";

interface Props {
  toggleFileIndexer: boolean;
  toggleAudioTranscription: boolean;
  onChange?: (setting: string, e: SettingChangeEvents) => void;
}

export function IndexFilesHelp({
  toggleFileIndexer,
  toggleAudioTranscription,
  onChange = () => {},
}: Props) {
  const [paths, setPaths] = useState<string[]>([]);
  useEffect(() => {
    const loadPaths = async () => {
      const defaults = await invoke<DefaultIndices>("default_indices");
      const paths = defaults.file_paths.sort();
      setPaths(paths);
    };
    loadPaths();
  }, []);

  const fileIndexerOpts: SettingOpts = {
    label: "Enable local file searching",
    value: JSON.stringify(toggleFileIndexer),
    form_type: "Bool",
    restart_required: false,
    help_text: null,
  };

  const toggleAudio: SettingOpts = {
    label: "Enable audio search",
    value: JSON.stringify(toggleAudioTranscription),
    form_type: "Bool",
    restart_required: false,
    help_text:
      "Search the audio content of podcasts, audio books, meetings, etc.",
  };

  return (
    <div className="p-4 bg-neutral-800 h-screen text-left text-neutral-400 flex flex-col gap-4">
      <h1 className="text-2xl flex flex-row items-center gap-2 text-white">
        <FileExtIcon className="w-8" filePath="any" />
        <div>Search your local files</div>
      </h1>
      <div className="text-sm">
        {
          "Enable local file search to index & search through markdown, word, excel, and other text based documents!"
        }
      </div>
      <FormElement
        className="flex flex-row"
        settingName="_.file-indexer"
        settingOptions={fileIndexerOpts}
        onChange={(e) => onChange("_.file-indexer", e)}
      />
      <FormElement
        className="flex flex-row"
        settingName="_.audio-transcription"
        settingOptions={toggleAudio}
        onChange={(e) => onChange("_.audio-transcription", e)}
      />
      <div className="text-sm">
        If enabled, the following folders will be automatically indexed. You can
        add/remove folders in your settings.
        <ul className="mt-4 text-sm text-cyan-500 flex flex-col gap-2 font-mono">
          {paths.map((path) => (
            <li key={path} className="list-disc">
              {path}
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
