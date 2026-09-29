import {
  Add01Icon,
  GithubIcon,
  Moon02Icon,
  Search01Icon,
  Sun03Icon,
} from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { memo } from 'react';
import { OctapusMark } from '@/components/octapus-mark';
import { Button } from '@/components/ui/button';
import { Kbd } from '@/components/ui/kbd';
import { Tooltip } from '@/components/ui/tooltip/tooltip';
import { ConnectionTree } from '@/features/connection-tree/connection-tree';
import { ServerForm } from '@/features/server-form/server-form';
import { shortcut } from '@/lib/platform';
import { useSidebar } from './use-sidebar';

export const Sidebar = memo(() => {
  const {
    isFormOpen,
    editingServer,
    serverCount,
    theme,
    openCreateForm,
    openEditForm,
    closeForm,
    openPalette,
    openRepository,
    toggleTheme,
  } = useSidebar();

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-12 shrink-0 items-center gap-2.5 pr-2.5 pl-3.5">
        <OctapusMark />
        <span className="text-heading font-semibold tracking-tight">
          Octapus
        </span>
        <span className="flex-1" />
        <Tooltip content="Adicionar servidor" position="bottom">
          <Button
            variant="ghost"
            size="icon"
            aria-label="Adicionar servidor"
            onClick={openCreateForm}
          >
            <HugeiconsIcon icon={Add01Icon} />
          </Button>
        </Tooltip>
      </div>

      {/* Atalho para a paleta: a busca de tabelas mora lá. */}
      <div className="shrink-0 px-2.5 pb-2.5">
        <button
          type="button"
          onClick={openPalette}
          className="flex h-[30px] w-full cursor-pointer items-center gap-2 rounded-[7px] bg-hover pr-1.5 pl-2.5 text-body text-fg-subtle shadow-[inset_0_0_0_1px_var(--line-subtle)] transition-colors outline-none hover:bg-active hover:text-fg-muted focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-offset-2 focus-visible:outline-ring"
        >
          <HugeiconsIcon icon={Search01Icon} className="size-[15px]" />
          <span className="flex-1 text-left">Buscar tabelas</span>
          <Kbd>{shortcut('K')}</Kbd>
        </button>
      </div>

      <div className="flex shrink-0 items-center justify-between px-4 py-1.5">
        <span className="text-micro font-medium uppercase text-fg-subtle">
          Conexões
        </span>
        <span className="text-[11px] tabular-nums text-fg-subtle">
          {serverCount}
        </span>
      </div>

      <div className="min-h-0 flex-1 px-2">
        <ConnectionTree onEditServer={openEditForm} />
      </div>

      <div className="flex h-11 shrink-0 items-center gap-1 border-t border-line-subtle pr-2 pl-3.5">
        <span className="font-mono text-[11px] text-fg-subtle">
          v{__APP_VERSION__}
        </span>
        <Tooltip content="Octapus no GitHub — deixe uma estrela" position="top">
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label="Abrir o repositório no GitHub"
            onClick={openRepository}
            className="text-fg-subtle hover:text-fg"
          >
            <HugeiconsIcon icon={GithubIcon} />
          </Button>
        </Tooltip>
        <span className="flex-1" />
        <Tooltip
          content={theme === 'dark' ? 'Tema claro' : 'Tema escuro'}
          position="top"
        >
          <Button
            variant="ghost"
            size="icon"
            aria-label={theme === 'dark' ? 'Usar tema claro' : 'Usar tema escuro'}
            onClick={toggleTheme}
          >
            <HugeiconsIcon icon={theme === 'dark' ? Sun03Icon : Moon02Icon} />
          </Button>
        </Tooltip>
      </div>

      <ServerForm
        open={isFormOpen}
        onClose={closeForm}
        server={editingServer}
      />
    </div>
  );
});

Sidebar.displayName = 'Sidebar';
