import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";

export const FALLBACK_APP_VERSION = "1.0.0";

let appVersionPromise: Promise<string> | undefined;

export const loadAppVersion = () => {
  appVersionPromise ??= getVersion().catch(() => FALLBACK_APP_VERSION);
  return appVersionPromise;
};

export const useAppVersion = () => {
  const [version, setVersion] = useState(FALLBACK_APP_VERSION);

  useEffect(() => {
    let active = true;
    void loadAppVersion().then((resolvedVersion) => {
      if (active) setVersion(resolvedVersion);
    });
    return () => {
      active = false;
    };
  }, []);

  return version;
};

const versionParts = (version: string) =>
  version
    .trim()
    .replace(/^v/i, "")
    .split("-", 1)[0]
    .split(".")
    .map((part) => Number.parseInt(part, 10) || 0);

export const compareVersions = (left: string, right: string) => {
  const leftParts = versionParts(left);
  const rightParts = versionParts(right);
  const length = Math.max(leftParts.length, rightParts.length);

  for (let index = 0; index < length; index += 1) {
    const difference = (leftParts[index] ?? 0) - (rightParts[index] ?? 0);
    if (difference !== 0) return difference;
  }

  return 0;
};
