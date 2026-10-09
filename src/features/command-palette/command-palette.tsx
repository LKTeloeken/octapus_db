import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from '@/components/ui/dialog';
import { PaletteContent } from './palette-content';
import { useCommandPalette } from './use-command-palette';

/**
 * Global Cmd/Ctrl+K table search across every registered server: Postgres from
 * the backend catalog (grouped across tenants), the others from the structure
 * cache. Mounted once at the app shell. The list is virtualized
 * and keyboard navigation is self-managed (see use-palette-navigation) so it
 * stays smooth with thousands of cached tables.
 *
 * The inner UI lives in PaletteContent, which only mounts while the dialog is
 * open — keeping the virtualizer's lifecycle tied to its scroll element.
 */
export function CommandPalette() {
  const {
    open,
    setOpen,
    query,
    setQuery,
    caret,
    rows,
    hasResults,
    connectingId,
    selectItem,
    isEmptyCache,
  } = useCommandPalette();

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent
        showCloseButton={false}
        // Vidro (DESIGN.md §7): camada temporária sobre a grade, blur 24 e
        // sobre o scrim do diálogo; opaca se o sistema reduzir transparência.
        className="top-[112px] translate-y-0 gap-0 overflow-hidden rounded-xl border-glass-border bg-glass p-0 shadow-glass backdrop-blur-[24px] backdrop-saturate-[1.4] reduced-transparency:bg-surface-3 reduced-transparency:backdrop-blur-none sm:max-w-[640px]"
      >
        <DialogTitle className="sr-only">Command Palette</DialogTitle>
        <DialogDescription className="sr-only">
          Buscar e executar comandos
        </DialogDescription>

        <PaletteContent
          query={query}
          setQuery={setQuery}
          caret={caret}
          rows={rows}
          hasResults={hasResults}
          connectingId={connectingId}
          selectItem={selectItem}
          isEmptyCache={isEmptyCache}
        />
      </DialogContent>
    </Dialog>
  );
}
