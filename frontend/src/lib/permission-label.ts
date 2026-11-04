const KIND_LABEL: Record<string, string> = {
  camera: "Camera",
  microphone: "Microphone",
  location: "Location",
  notifications: "Notifications",
  clipboard: "Clipboard",
  midi: "MIDI devices",
  "window-management": "Windows",
  "local-fonts": "Fonts",
  "file-system": "Files",
  autoplay: "Sound",
  sensors: "Motion sensors",
  downloads: "Downloads",
  storage: "Stored data",
};

const PROTOCOL_LABEL: Record<string, string> = {
  mailto: "Email links",
  webcal: "Calendar links",
  tel: "Phone links",
  sms: "Text message links",
  smsto: "Text message links",
  geo: "Map links",
  magnet: "Magnet links",
};

/** Short name for a stored permission, including `protocol:mailto`. */
export function permissionLabel(kind: string) {
  if (KIND_LABEL[kind]) {
    return KIND_LABEL[kind];
  }
  if (kind.startsWith("protocol:")) {
    const scheme = kind.slice("protocol:".length);
    return PROTOCOL_LABEL[scheme] ?? `${scheme} links`;
  }
  return kind;
}
