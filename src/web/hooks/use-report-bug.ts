import { openUrl } from "@tauri-apps/plugin-opener";

const RELEASE_REPO = "wunderlabs-dev/openable";

const buildBody = (version: string, platform: string) =>
  [
    "<!-- One short sentence describing what happened. -->",
    "",
    "## Steps to reproduce",
    "1. ",
    "",
    "## Expected",
    "",
    "## Actual",
    "",
    "## Diagnostics",
    "Attach the zip from Settings -> Diagnostics -> Export.",
    "",
    "## Environment",
    `- Opnble: ${version}`,
    `- Platform: ${platform}`,
  ].join("\n");

const useReportBug = (version: string, platform: string) => {
  return () => {
    const title = encodeURIComponent("[Beta] ");
    const body = encodeURIComponent(buildBody(version, platform));
    openUrl(
      `https://github.com/${RELEASE_REPO}/issues/new?template=bug.yml&title=${title}&body=${body}`,
    );
  };
};

export { useReportBug };
