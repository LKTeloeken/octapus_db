import { HugeiconsIcon } from '@hugeicons/react';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from '@/components/ui/dialog';
import { cn } from '@/lib/utils';
import type { SettingsDialogProps } from './settings-dialog.types';
import { useSettingsDialog } from './use-settings-dialog';

/**
 * Configurações do app: menu de seções à esquerda, a seção ativa à direita.
 * A altura é fixa para o diálogo não pular ao trocar de seção; o conteúdo rola.
 */
export function SettingsDialog({ open, onOpenChange }: SettingsDialogProps) {
  const { sections, active, setActiveId } = useSettingsDialog();
  const Section = active.Component;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex h-[440px] gap-0 overflow-hidden p-0 sm:max-w-[680px]">
        <nav
          aria-label="Seções das configurações"
          className="flex w-48 shrink-0 flex-col gap-0.5 border-r border-line-subtle bg-surface-2 p-2"
        >
          <DialogTitle className="px-2.5 pt-2 pb-3 text-heading">
            Configurações
          </DialogTitle>
          {sections.map(section => {
            const isActive = section.id === active.id;
            return (
              <button
                key={section.id}
                type="button"
                aria-current={isActive ? 'page' : undefined}
                onClick={() => setActiveId(section.id)}
                className={cn(
                  'flex h-7 cursor-pointer items-center gap-2 rounded-sm px-2.5 text-body transition-colors outline-none focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-offset-[-2px] focus-visible:outline-ring',
                  isActive
                    ? 'bg-active text-fg'
                    : 'text-fg-muted hover:bg-hover hover:text-fg',
                )}
              >
                <HugeiconsIcon
                  icon={section.icon}
                  className="size-4 shrink-0"
                />
                {section.label}
              </button>
            );
          })}
        </nav>

        <div className="flex min-w-0 flex-1 flex-col overflow-y-auto scrollbar-thin px-6 pt-5 pb-6">
          <h3 className="pr-8 text-title font-semibold">{active.label}</h3>
          <DialogDescription className="mt-1">
            {active.description}
          </DialogDescription>
          <div className="mt-5 flex flex-col">
            <Section />
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
