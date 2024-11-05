import { ShareIcon } from "@heroicons/react/24/solid";
import {
  SiGithub,
  SiGmail,
  SiGooglecalendar,
  SiGoogledrive,
  SiReddit,
} from "@icons-pack/react-simple-icons";

interface Props {
  connection: string;
  className: string;
}

export function ConnectionIcon({ connection, className }: Props) {
  switch (connection) {
    case "calendar.google.com":
      return <SiGooglecalendar className={className} />;
    case "drive.google.com":
      return <SiGoogledrive className={className} />;
    case "mail.google.com":
      return <SiGmail className={className} />;
    case "api.github.com":
      return <SiGithub className={className} />;
    case "oauth.reddit.com":
      return <SiReddit className={className} />;
    default:
      return <ShareIcon className={className} />;
  }
}