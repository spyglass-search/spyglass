import {
  InvokeArgs,
  InvokeOptions,
  invoke as tauriInvoke,
} from "@tauri-apps/api/core";
import {
  EventCallback,
  Options,
  listen as tauriListen,
  UnlistenFn,
} from "@tauri-apps/api/event";
import { ClientInvoke } from "./bindings/ClientInvoke";
import { ClientEvent } from "./bindings/ClientEvent";

export async function invoke<T>(
  cmd: ClientInvoke,
  args?: InvokeArgs,
  opts?: InvokeOptions,
): Promise<T> {
  return tauriInvoke(cmd, args, opts);
}

// Some wrappers around tauri functions to make them more type aware.
export async function listen(
  event: ClientEvent,
  handler: EventCallback<any>,
  opts?: Options,
): Promise<UnlistenFn> {
  return tauriListen(event, handler, opts);
}
