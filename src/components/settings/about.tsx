import { Button } from "@/components/ui/button";
import { Item, ItemActions, ItemContent, ItemDescription, ItemTitle } from "@/components/ui/item";
import { compareVersions, loadAppVersion, useAppVersion } from "@/lib/app-version";
import { useTranslation } from "@/lib/i18n";
import { GithubIcon, LinkSquare02Icon, WebDesign01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { CheckCircle2, Download, RefreshCw, TriangleAlert } from "lucide-react";
import { useState } from "react";

const LATEST_RELEASE_API =
    "https://api.github.com/repos/harmonica80/keyviz-multi-monitor/releases/latest";

interface GithubRelease {
    tag_name: string;
    html_url: string;
}

type UpdateStatus =
    | { state: "idle" }
    | { state: "checking" }
    | { state: "current"; latestVersion: string }
    | { state: "available"; latestVersion: string; url: string }
    | { state: "error" };

export const AboutPage = () => {
    const { t } = useTranslation();
    const version = useAppVersion();
    const [updateStatus, setUpdateStatus] = useState<UpdateStatus>({ state: "idle" });

    const checkForUpdates = async () => {
        setUpdateStatus({ state: "checking" });
        const controller = new AbortController();
        const timeout = window.setTimeout(() => controller.abort(), 10_000);

        try {
            const currentVersion = await loadAppVersion();
            const response = await fetch(LATEST_RELEASE_API, {
                headers: { Accept: "application/vnd.github+json" },
                signal: controller.signal,
            });
            if (!response.ok) throw new Error(`GitHub returned ${response.status}`);

            const release = await response.json() as GithubRelease;
            const latestVersion = release.tag_name.replace(/^v/i, "");
            if (compareVersions(latestVersion, currentVersion) > 0) {
                setUpdateStatus({
                    state: "available",
                    latestVersion,
                    url: release.html_url,
                });
            } else {
                setUpdateStatus({ state: "current", latestVersion });
            }
        } catch (error) {
            console.error("Failed to check for updates:", error);
            setUpdateStatus({ state: "error" });
        } finally {
            window.clearTimeout(timeout);
        }
    };

    return (
        <div>
            <div className="flex flex-col items-center bg-linear-to-b from-secondary to-background py-8">
                <img className="h-24 w-24" src="./logo.svg" alt="Keyviz" />
                <h1 className="mb-1 mt-4 text-xl font-semibold">
                    {t("Keyviz Keyboard Visualizer")}
                </h1>
                <p className="text-center text-sm text-muted-foreground">
                    v{version}
                    <br />
                    © 2026 {t("Teacher Chiu Learning Website")}
                </p>
            </div>

            <div className="mt-6 flex flex-col gap-4 px-6">
                <Item variant="muted">
                    <ItemContent>
                        <ItemTitle>
                            <HugeiconsIcon icon={GithubIcon} size="1em" />
                            {t("Keyviz Open Source Website")}
                        </ItemTitle>
                        <ItemDescription className="max-w-100">
                            https://github.com/harmonica80/keyviz-multi-monitor
                        </ItemDescription>
                    </ItemContent>
                    <ItemActions>
                        <Button
                            variant="outline"
                            size="icon"
                            aria-label={t("Open Keyviz source code")}
                            onClick={() => openUrl("https://github.com/harmonica80/keyviz-multi-monitor")}
                        >
                            <HugeiconsIcon icon={LinkSquare02Icon} />
                        </Button>
                    </ItemActions>
                </Item>

                <Item variant="muted">
                    <ItemContent>
                        <ItemTitle>
                            <HugeiconsIcon icon={WebDesign01Icon} size="1em" />
                            {t("Teacher Chiu Learning Website")}
                        </ItemTitle>
                        <ItemDescription className="max-w-100">
                            https://harmonica80.blogspot.com/
                        </ItemDescription>
                    </ItemContent>
                    <ItemActions>
                        <Button
                            variant="outline"
                            size="icon"
                            aria-label={t("Open Teacher Chiu Learning Website")}
                            onClick={() => openUrl("https://harmonica80.blogspot.com/")}
                        >
                            <HugeiconsIcon icon={LinkSquare02Icon} />
                        </Button>
                    </ItemActions>
                </Item>

                <Item variant="muted">
                    <ItemContent>
                        <ItemTitle>
                            {updateStatus.state === "available" ? (
                                <Download className="size-4" />
                            ) : updateStatus.state === "error" ? (
                                <TriangleAlert className="size-4" />
                            ) : updateStatus.state === "current" ? (
                                <CheckCircle2 className="size-4" />
                            ) : (
                                <RefreshCw className={`size-4 ${updateStatus.state === "checking" ? "animate-spin" : ""}`} />
                            )}
                            {t("Check for Updates")}
                        </ItemTitle>
                        <ItemDescription className="max-w-100">
                            {updateStatus.state === "idle" &&
                                t("Current version: v{version}", { version })}
                            {updateStatus.state === "checking" && t("Checking for updates...")}
                            {updateStatus.state === "current" &&
                                t("You're using the latest version (v{version}).", {
                                    version: updateStatus.latestVersion,
                                })}
                            {updateStatus.state === "available" &&
                                t("A new version is available: v{version}", {
                                    version: updateStatus.latestVersion,
                                })}
                            {updateStatus.state === "error" &&
                                t("Unable to check for updates. Please check your internet connection and try again.")}
                        </ItemDescription>
                    </ItemContent>
                    <ItemActions>
                        {updateStatus.state === "available" ? (
                            <Button
                                variant="outline"
                                size="sm"
                                onClick={() => openUrl(updateStatus.url)}
                            >
                                <Download />
                                {t("Download")}
                            </Button>
                        ) : (
                            <Button
                                variant="outline"
                                size="sm"
                                disabled={updateStatus.state === "checking"}
                                onClick={checkForUpdates}
                            >
                                <RefreshCw className={updateStatus.state === "checking" ? "animate-spin" : ""} />
                                {t(updateStatus.state === "checking" ? "Checking..." : "Check")}
                            </Button>
                        )}
                    </ItemActions>
                </Item>
            </div>
        </div>
    );
};
