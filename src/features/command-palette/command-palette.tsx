import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from '@/components/ui/dialog';
import { PaletteContent } from './palette-content';
import { useCommandPalette } from './use-command-palette';

/**
 * Global Cmd/Ctrl+K table search across every registered server, sourced from
 * the structure cache. Mounted once at the app shell. The list is virtualized
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
    rows,
    hasResults,
    connectingId,
    selectEntry,
    isEmptyCache,
  } = useCommandPalette();

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent
        showCloseButton={false}
        className="overflow-hidden p-0 sm:max-w-xl"
      >
        <DialogTitle className="sr-only">Command Palette</DialogTitle>
        <DialogDescription className="sr-only">
          Buscar e executar comandos
        </DialogDescription>

        <PaletteContent
          query={query}
          setQuery={setQuery}
          rows={rows}
          hasResults={hasResults}
          connectingId={connectingId}
          selectEntry={selectEntry}
          isEmptyCache={isEmptyCache}
        />
      </DialogContent>
    </Dialog>
  );
}
