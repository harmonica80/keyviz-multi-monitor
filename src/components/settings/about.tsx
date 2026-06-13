import { Button } from "@/components/ui/button";
import { Item, ItemActions, ItemContent, ItemDescription, ItemTitle } from "@/components/ui/item";
import { GithubIcon, LinkSquare02Icon, WebDesign01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { openUrl } from "@tauri-apps/plugin-opener";

export const VERSION = "1.0";

export const AboutPage = () => {
    return (
        <div>
            <div className="flex flex-col items-center bg-linear-to-b from-secondary to-background py-8">
                <img className="h-24 w-24" src="./logo.svg" alt="Keyviz" />
                <h1 className="mb-1 mt-4 text-xl font-semibold">
                    Keyviz 鍵盤按鍵顯示器（支援多螢幕）
                </h1>
                <p className="text-center text-sm text-muted-foreground">
                    v{VERSION}
                    <br />
                    © 2026 述文老師學習網
                </p>
            </div>

            <div className="mt-6 flex flex-col gap-4 px-6">
                <Item variant="muted">
                    <ItemContent>
                        <ItemTitle>
                            <HugeiconsIcon icon={GithubIcon} size="1em" />
                            Keyviz 開放原始碼網站
                        </ItemTitle>
                        <ItemDescription className="max-w-100">
                            https://github.com/harmonica80/keyviz-multi-monitor
                        </ItemDescription>
                    </ItemContent>
                    <ItemActions>
                        <Button
                            variant="outline"
                            size="icon"
                            aria-label="開啟 Keyviz 原始碼網站"
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
                            述文老師學習網
                        </ItemTitle>
                        <ItemDescription className="max-w-100">
                            https://harmonica80.blogspot.com/
                        </ItemDescription>
                    </ItemContent>
                    <ItemActions>
                        <Button
                            variant="outline"
                            size="icon"
                            aria-label="開啟述文老師學習網"
                            onClick={() => openUrl("https://harmonica80.blogspot.com/")}
                        >
                            <HugeiconsIcon icon={LinkSquare02Icon} />
                        </Button>
                    </ItemActions>
                </Item>
            </div>
        </div>
    );
};
