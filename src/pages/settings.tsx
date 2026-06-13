import { useEffect, useState } from "react";

import { AboutPage, AppearanceSettings, GeneralSettings, KeycapSettings, MouseSettings } from "@/components/settings";
import { VERSION } from "@/components/settings/about";
import { ThemeModeToggle } from "@/components/theme-mode-toggle";
import { LocaleToggle } from "@/components/locale-toggle";
import { useTranslation } from "@/lib/i18n";
import { invoke } from "@tauri-apps/api/core";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { SidebarItem } from "@/components/ui/sidebar-item";
import { ComputerIcon, InformationSquareIcon, KeyboardIcon, Mouse09Icon, Settings03Icon } from "@hugeicons/core-free-icons";

const sideBar = [
    { id: "General", icon: Settings03Icon },
    { id: "Appearance", icon: ComputerIcon },
    { id: "Keycap", icon: KeyboardIcon },
    { id: "Mouse", icon: Mouse09Icon },
]

const Settings = () => {
    const [activeTab, setActiveTab] = useState(sideBar[0].id);
    const { locale, t } = useTranslation();

    useEffect(() => {
        document.documentElement.lang = locale;
        invoke("set_tray_locale", { locale }).catch((error) => {
            console.error("Failed to update tray locale:", error);
        });
    }, [locale]);

    return (
        <div className="flex w-screen h-screen overflow-hidden border-t bg-background">
            <div className="w-44 p-2 flex flex-col gap-y-1 rounded-xl">
                <div className="flex items-center m-2 mb-2 gap-x-2">
                    <img src="./logo.svg" alt="logo" className="w-8 h-8" />
                    <div className="flex flex-col gap-y-0.5">
                        <h1 className="text-sm font-semibold">Keyviz 鍵盤按鍵顯示器（支援多螢幕）</h1>
                        <p className="text-xs text-gray-400">v{VERSION}</p>
                    </div>
                </div>
                {
                    sideBar.map((item) => (
                        <a key={item.id} onClick={() => setActiveTab(item.id)} className="cursor-pointer">
                            <SidebarItem item={{ title: t(item.id), icon: item.icon }} isActive={activeTab === item.id} />
                        </a>
                    ))
                }
                <div className="mt-auto flex gap-2 items-center">
                    <a key="about" onClick={() => setActiveTab("About")} className="flex-1 cursor-pointer">
                        <SidebarItem item={{ title: t("About"), icon: InformationSquareIcon }} isActive={activeTab === "About"} />
                    </a>
                    <LocaleToggle />
                    <ThemeModeToggle />
                </div>
            </div>
            <Separator orientation="vertical" />
            <ScrollArea className="flex-1 relative">
                {activeTab === "General" && <GeneralSettings />}
                {activeTab === "Appearance" && <AppearanceSettings />}
                {activeTab === "Keycap" && <KeycapSettings />}
                {activeTab === "Mouse" && <MouseSettings />}
                {activeTab === "About" && <AboutPage />}
            </ScrollArea>
        </div>
    );
}

export default Settings;
