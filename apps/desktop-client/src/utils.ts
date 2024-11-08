export enum OperatingSystem {
  MacOS,
  Windows,
  Linux,
}

export function getOperatingSystem(): OperatingSystem {
  const userAgent = navigator.userAgent.toLowerCase();
  if (userAgent.includes("macintosh")) {
    return OperatingSystem.MacOS;
  } else if (userAgent.includes("microsoft")) {
    return OperatingSystem.Windows;
  } else {
    return OperatingSystem.Linux;
  }
}
