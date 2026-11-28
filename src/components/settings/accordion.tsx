import { useState, type ReactNode } from "react";
import { ChevronRight } from "lucide-react";

import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { cn } from "@/lib/utils";

/** Accordion state: at most one section is open; opening another closes the current one. */
export const useAccordion = (initial: string | null) => {
  const [openId, setOpenId] = useState<string | null>(initial);
  return {
    isOpen: (id: string) => openId === id,
    setOpen: (id: string, open: boolean) => setOpenId((current) => (open ? id : current === id ? null : current)),
  };
};

interface AccordionSectionProps {
  title: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  children: ReactNode;
}

/** Settings section with a leading chevron that folds the content. */
export const AccordionSection = ({ title, open, onOpenChange, children }: AccordionSectionProps) => (
  <Collapsible open={open} onOpenChange={onOpenChange}>
    <CollapsibleTrigger className="flex w-full cursor-pointer items-center gap-1.5 text-left">
      <ChevronRight
        className={cn("size-4 shrink-0 text-muted-foreground transition-transform", open && "rotate-90")}
        aria-hidden="true"
      />
      <h2 className="text-sm text-muted-foreground font-medium">{title}</h2>
    </CollapsibleTrigger>
    <CollapsibleContent className="flex flex-col gap-y-4 pt-4">{children}</CollapsibleContent>
  </Collapsible>
);
