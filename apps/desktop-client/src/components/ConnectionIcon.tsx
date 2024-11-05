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
// pub fn connection_icon(id: &str, height: &str, width: &str, classes: Classes) -> Html {
//     let height = height.to_string();
//     let width = width.to_string();

//     if id == "calendar.google.com" {
//         html! { <GoogleCalendar {height} {width} {classes} /> }
//     } else if id == "drive.google.com" {
//         html! { <GDrive {height} {width} {classes} /> }
//     } else if id == "mail.google.com" {
//         html! { <Gmail {height} {width} {classes} /> }
//     } else if id == "api.github.com" {
//         html! { <GitHub {height} {width} {classes} /> }
//     } else if id == "oauth.reddit.com" {
//         html! { <Reddit {height} {width} {classes} /> }
//     } else {
//         html! { <ShareIcon {height} {width} {classes} /> }
//     }
// }
