import { Button } from "@/components/ui/button";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useLocale, useTranslation } from "@/lib/i18n";
import { invoke } from "@tauri-apps/api/core";

export function LocaleToggle() {
    const setLocale = useLocale((state) => state.setLocale);
    const { locale, t } = useTranslation();
    const selectLocale = (nextLocale: "en" | "zh-TW") => {
        setLocale(nextLocale);
        invoke("set_tray_locale", { locale: nextLocale }).catch((error) => {
            console.error("Failed to update tray locale:", error);
        });
    };

    return (
        <DropdownMenu>
            <DropdownMenuTrigger asChild>
                <Button variant="ghost" size="icon" title={t("Language")}>
                    <span className="text-xs font-semibold text-muted-foreground">
                        {locale === "en" ? "EN" : "中"}
                    </span>
                    <span className="sr-only">{t("Language")}</span>
                </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start">
                <DropdownMenuItem
                    onClick={() => selectLocale("en")}
                    className={locale === "en" ? "font-semibold" : ""}
                >
                    English
                </DropdownMenuItem>
                <DropdownMenuItem
                    onClick={() => selectLocale("zh-TW")}
                    className={locale === "zh-TW" ? "font-semibold" : ""}
                >
                    繁體中文
                </DropdownMenuItem>
            </DropdownMenuContent>
        </DropdownMenu>
    );
}
