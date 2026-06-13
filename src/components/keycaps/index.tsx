import { KeyEvent } from "@/types/event";
import { ModernKeycap } from "./modern";

export interface KeycapProps {
    event: KeyEvent;
    isPressed: boolean;
    lastest: boolean;
}

export const Keycap = (props: KeycapProps) => {
    return <ModernKeycap {...props} />;
};
