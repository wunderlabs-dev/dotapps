import { useMutation } from "@tanstack/react-query";
import { useCallback, useState } from "react";
import { describeError } from "@/components/launcher/launcher-error";
import { useToastContext } from "@/context";
import { dotappsApi } from "@/lib/dotapps";
import { parseDotappsUri } from "@/lib/dotapps-uri";

const useLauncherUri = (onChanged: () => void) => {
  const { showToast } = useToastContext();
  const [uri, setUri] = useState("");

  const install = useMutation({
    mutationFn: async (raw: string) => {
      const link = parseDotappsUri(raw);
      if (!link) {
        throw new Error("Enter a dotapps:// link");
      }
      await dotappsApi.install(link.slug, link.version);
      await dotappsApi.run(link.slug);
      await dotappsApi.open(link.slug);
      return link.slug;
    },
    onSuccess: () => {
      setUri("");
      showToast("App installed", "success");
      onChanged();
    },
    onError: (error) => {
      showToast(describeError(error), "error");
    },
  });

  const submitUri = useCallback(() => {
    if (!uri.trim() || install.isPending) {
      return;
    }
    install.mutate(uri);
  }, [install, uri]);

  return {
    uri,
    setUri,
    submitUri,
    submitting: install.isPending,
  };
};

export { useLauncherUri };
