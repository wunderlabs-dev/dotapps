import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";

const openLink = (url: string) => {
  openUrl(url);
};

const revealPath = (path: string) => {
  revealItemInDir(path);
};

export { openLink, revealPath };
