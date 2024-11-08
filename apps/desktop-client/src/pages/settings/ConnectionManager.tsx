import { ReactNode, useEffect, useState } from "react";
import { Btn } from "../../components/Btn";
import { Header } from "./Header";
import {
  ArrowPathIcon,
  BoltIcon,
  PlusCircleIcon,
  ShareIcon,
  TrashIcon,
  XMarkIcon,
} from "@heroicons/react/24/solid";
import { invoke, listen } from "../../glue";
import { ListConnectionResult } from "../../bindings/ListConnectionResult";
import { UserConnection } from "../../bindings/UserConnection";
import { SupportedConnection } from "../../bindings/SupportedConnection";
import {
  SiGithub,
  SiGmail,
  SiGooglecalendar,
  SiGoogledrive,
  SiReddit,
} from "@icons-pack/react-simple-icons";
import googleSignin from "../../assets/google_signin.png";
import googleSigninDisabled from "../../assets/google_signin_disabled.png";
import { BtnType } from "../../components/_constants";
import classNames from "classnames";

enum RequestState {
  NotStarted,
  InProgress,
  Finished,
  Error,
}

function getConnectionIcon(id: string, className?: string): ReactNode {
  if (id == "calendar.google.com") {
    return <SiGooglecalendar className={className} />;
  } else if (id == "drive.google.com") {
    return <SiGoogledrive className={className} />;
  } else if (id == "mail.google.com") {
    return <SiGmail className={className} />;
  } else if (id == "api.github.com") {
    return <SiGithub className={className} />;
  } else if (id == "oauth.reddit.com") {
    return <SiReddit className={className} />;
  } else {
    return <ShareIcon className={className} />;
  }
}

interface ConnectionBtnProps {
  id: string;
  disabled: boolean;
  onClick?: () => void;
}

function ConnectionBtn({
  id,
  disabled,
  onClick = () => {},
}: ConnectionBtnProps): ReactNode {
  // Annoyingly we need to use Google branded icons for the connection
  // button.
  if (id.endsWith("google.com")) {
    return (
      <button disabled={disabled} onClick={onClick} className="ml-auto">
        {disabled ? (
          <img src={googleSigninDisabled} className="w-auto h-10" />
        ) : (
          <img src={googleSignin} className="w-auto h-10" />
        )}
      </button>
    );
  } else {
    return (
      <Btn
        disabled={disabled}
        onClick={onClick}
        className="ml-auto btn-neutral"
      >
        <BoltIcon className="mr-2 w-4" />
        Connect
      </Btn>
    );
  }
}

interface UserConnnectionProps {
  connection: UserConnection;
  label: string;
}
function UserConnectionInfo({ connection, label }: UserConnnectionProps) {
  const [isRevoking, setIsRevoking] = useState<boolean>(false);
  const [isResyncing, setIsResyncing] = useState<boolean>(false);

  const handleResync = async () => {
    setIsResyncing(true);
    await invoke("resync_connection", {
      id: connection.id,
      account: connection.account,
    });
    // TODO: listen for resync finished events.
  };

  const handleRevoke = async () => {
    setIsRevoking(true);
    await invoke("revoke_connection", {
      id: connection.id,
      account: connection.account,
    });
    // TODO: listen for revoke finished events.
  };

  return (
    <div className="rounded-md bg-neutral-700 p-4 text-white shadow-md flex flex-row gap-4 items-center">
      <div>{getConnectionIcon(connection.id, "w-6")}</div>
      <div>
        <div className="text-xs font-bold text-cyan-500">{label}</div>
        <div className="text-sm">{connection.account}</div>
      </div>
      <div className="grow flex flex-row gap-4 place-content-end">
        <Btn
          disabled={isResyncing}
          onClick={handleResync}
          className="btn-sm text-sm"
        >
          <ArrowPathIcon
            className={classNames("w-4", { "animate-spin": isResyncing })}
          />
          {isResyncing ? "Syncing" : "Resync"}
        </Btn>
        <Btn
          disabled={isRevoking}
          type={BtnType.Danger}
          className="btn-sm text-sm"
          onClick={handleRevoke}
        >
          {isRevoking ? (
            <>
              <ArrowPathIcon className="w-4 animate-spin" />
              Deleting
            </>
          ) : (
            <>
              <TrashIcon className="w-4" />
              Delete
            </>
          )}
        </Btn>
      </div>
    </div>
  );
}

export function ConnectionManager() {
  const [connectionState, setConnectionState] = useState<RequestState>(
    RequestState.InProgress,
  );
  const [userConnections, setUserConnections] = useState<UserConnection[]>([]);
  const [supportedConns, setSuppportedConns] = useState<SupportedConnection[]>(
    [],
  );

  const [isAdding, setIsAdding] = useState<boolean>(false);
  const [authorizationState, setAuthorizationState] = useState<RequestState>(
    RequestState.NotStarted,
  );
  const [authError, setAuthError] = useState<string>("");

  const handleAdd = async (id: string) => {
    setAuthorizationState(RequestState.InProgress);
    await invoke("authorize_connection", { id })
      .then(() => {
        setAuthorizationState(RequestState.Finished);
        refreshConnections();
      })
      .catch((err) => {
        console.error(err);
        setAuthorizationState(RequestState.Error);
        setAuthError(err);
      });
  };

  const refreshConnections = async () => {
    const conns = await invoke<ListConnectionResult>("list_connections");
    setConnectionState(RequestState.Finished);
    setSuppportedConns(
      conns.supported.sort((a, b) => a.label.localeCompare(b.label)),
    );
    setUserConnections(
      conns.user_connections.sort((a, b) => a.account.localeCompare(b.account)),
    );
    setIsAdding(conns.user_connections.length === 0);
  };

  useEffect(() => {
    const initialize = async () => {
      await refreshConnections();
      return await listen("RefreshConnections", refreshConnections);
    };

    const listenRes = initialize().catch(console.error);
    return () => {
      (async () => {
        await listenRes.then((unlisten) => (unlisten ? unlisten() : null));
      })();
    };
  }, []);

  const renderAuthState = () => {
    if (authorizationState === RequestState.Error) {
      return <span className="text-error">{authError}</span>;
    } else if (authorizationState === RequestState.InProgress) {
      return (
        <span className="text-cyan-500">
          <div>
            Sign-in has opened in a new window. Please authorize to complete
            connection.
          </div>
        </span>
      );
    }
    return null;
  };

  return (
    <div>
      <Header label="Connections">
        {isAdding ? (
          userConnections.length > 0 ? (
            <Btn
              onClick={() => setIsAdding(false)}
              type={BtnType.Danger}
              className="btn-sm"
            >
              <XMarkIcon className="w-4" />
              Cancel
            </Btn>
          ) : null
        ) : (
          <Btn
            onClick={() => setIsAdding(true)}
            type={BtnType.Primary}
            className="btn-sm"
          >
            <PlusCircleIcon className="w-4" />
            Add
          </Btn>
        )}
      </Header>
      <div className="flex flex-col gap-4 px-8">
        {connectionState === RequestState.InProgress ? (
          <div className="flex justify-center">
            <div className="p-16">
              <ArrowPathIcon className="w-16 animate-spin" />
            </div>
          </div>
        ) : null}
        {isAdding ? (
          <div className="bg-neutral-800">
            <div className="mb-4 text-sm">{renderAuthState()}</div>
            {supportedConns.map((conn) => (
              <div
                key={conn.id}
                className="pb-8 flex flex-row items-center gap-8"
              >
                <div className="flex-none">
                  {getConnectionIcon(conn.id, "w-6")}
                </div>
                <div className="flex-1">
                  <div>
                    <h2 className="text-lg">{conn.label}</h2>
                  </div>
                  <div className="text-xs text-neutral-400">
                    {conn.description}
                  </div>
                </div>
                <div className="flex-none flex flex-col">
                  <ConnectionBtn
                    id={conn.id}
                    disabled={authorizationState === RequestState.InProgress}
                    onClick={() => handleAdd(conn.id)}
                  />
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="pt-4">
            {userConnections.map((conn) => (
              <UserConnectionInfo
                connection={conn}
                label={
                  supportedConns.find((x) => x.id === conn.id)?.label ??
                  "Connection"
                }
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
