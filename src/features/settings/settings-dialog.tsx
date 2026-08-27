import { HugeiconsIcon } from '@hugeicons/react';
import { SimpleDialog } from '@/components/ui/simple-dialog';
import { cn } from '@/lib/utils';
import type { SettingsDialogProps } from './settings.types';
import { useSettings } from './use-settings';

export function SettingsDialog({ open, onClose }: SettingsDialogProps) {
  const { sections, active, setActiveId } = useSettings({ open });

  return (
    <SimpleDialog open={open} onOpenChange={onClose} title="Configurações">
      <div className="flex min-h-72 gap-4">
        {/* A navegação só aparece quando há mais de uma seção — enquanto o app
            tiver uma só, uma coluna vazia seria ruído. */}
        {sections.length > 1 && (
          <nav className="flex w-40 shrink-0 flex-col gap-1">
            {sections.map(section => (
              <button
                key={section.id}
                type="button"
                onClick={() => setActiveId(section.id)}
                className={cn(
                  'flex items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm',
                  'hover:bg-accent hover:text-accent-foreground',
                  section.id === active.id &&
                    'bg-accent text-accent-foreground font-medium',
                )}
              >
                <HugeiconsIcon icon={section.icon} size={16} />
                {section.label}
              </button>
            ))}
          </nav>
        )}

        <div className="flex-1 overflow-y-auto">
          <active.Component />
        </div>
      </div>
    </SimpleDialog>
  );
}
